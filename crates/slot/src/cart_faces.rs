use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;

use slot_gfx::{Compositor, TexId};
use slot_store::Cart;
use slot_ui::{cart_face_worn, cart_interior, shows_interior, CartFace};

use crate::app::{App, FaceWant};
use crate::label_cache::label_art;

const WORKERS: usize = 2;

type Key = (usize, usize);

/// A cart's face and, for a clear GBA cart, the game-coloured inside it is drawn over.
type Built = (Key, CartFace, Option<CartFace>);

#[derive(Default)]
struct Queue {
    jobs: VecDeque<(Key, Cart, f32)>,
    closed: bool,
}

type Shared = Arc<(Mutex<Queue>, Condvar)>;

#[derive(Default)]
struct Building {
    carts: HashMap<Key, (Cart, u32)>,
    outdated: HashSet<Key>,
}

impl Building {
    fn start(&mut self, key: Key, cart: &Cart, wear: f32) -> bool {
        match self.carts.get(&key) {
            None => {
                self.carts.insert(key, (cart.clone(), wear.to_bits()));
                true
            }
            Some((building, worn)) => {
                if building != cart || *worn != wear.to_bits() {
                    self.outdated.insert(key);
                }
                false
            }
        }
    }

    fn unqueue(&mut self, key: Key) {
        self.carts.remove(&key);
        self.outdated.remove(&key);
    }

    fn finish(&mut self, key: Key, seated: Option<&Cart>) -> bool {
        let built = self.carts.remove(&key);
        !self.outdated.remove(&key) && built.as_ref().map(|(cart, _)| cart) == seated
    }
}

pub struct CartFaces {
    queue: Shared,
    built: Receiver<Built>,
    building: Building,
    asked: Vec<FaceWant>,
    worn: Vec<Key>,
    free: Vec<TexId>,
}

impl CartFaces {
    pub fn spawn(root: PathBuf) -> Self {
        let queue: Shared = Arc::default();
        let (outbox, built) = mpsc::channel();
        for n in 0..WORKERS {
            let (queue, outbox, root) = (queue.clone(), outbox.clone(), root.clone());
            let spawned = thread::Builder::new()
                .name(format!("slot-carts-{n}"))
                .spawn(move || work(&queue, &outbox, &root));
            if let Err(e) = spawned {
                eprintln!("slot: cart faces: worker thread failed to start: {e}");
            }
        }
        CartFaces {
            queue,
            built,
            building: Building::default(),
            asked: Vec::new(),
            worn: Vec::new(),
            free: Vec::new(),
        }
    }

    pub fn sync(&mut self, app: &mut App, compositor: &mut Compositor) {
        self.free.extend(app.shed_faces());
        while let Ok((key, face, interior)) = self.built.try_recv() {
            self.place(app, compositor, key, face, interior);
        }
        let wants = app.face_wants();
        let worn = app.take_worn();
        if wants != self.asked || !worn.is_empty() {
            self.worn.extend(worn);
            self.requeue(app, &wants);
            self.asked = wants;
        }
        let mut waiting: Vec<Key> = self
            .asked
            .iter()
            .filter(|w| w.on_screen)
            .map(|w| (w.shelf, w.index))
            .collect();
        while !waiting.is_empty() {
            let Ok((key, face, interior)) = self.built.recv() else {
                return;
            };
            waiting.retain(|k| *k != key);
            self.place(app, compositor, key, face, interior);
        }
    }

    fn requeue(&mut self, app: &App, wants: &[FaceWant]) {
        let (lock, wake) = &*self.queue;
        let mut queue = lock.lock().unwrap_or_else(|e| e.into_inner());
        for (key, _, _) in queue.jobs.drain(..) {
            self.building.unqueue(key);
        }
        let keys = wants.iter().map(|w| (w.shelf, w.index));
        for key in keys.chain(self.worn.iter().copied()) {
            if let Some(cart) = app.face_cart(key.0, key.1) {
                let wear = app.cart_wear(&cart.stem);
                if self.building.start(key, cart, wear) {
                    queue.jobs.push_back((key, cart.clone(), wear));
                }
            }
        }
        wake.notify_all();
    }

    fn texture(&mut self, compositor: &mut Compositor, face: &CartFace) -> TexId {
        match self.free.pop() {
            Some(tex) => {
                compositor.update_texture(tex, face.w, face.h, &face.rgba);
                tex
            }
            None => compositor.create_texture(face.w, face.h, &face.rgba),
        }
    }

    fn place(
        &mut self,
        app: &mut App,
        compositor: &mut Compositor,
        key: Key,
        face: CartFace,
        interior: Option<CartFace>,
    ) {
        self.asked.retain(|w| (w.shelf, w.index) != key);
        if !self.building.finish(key, app.face_cart(key.0, key.1)) {
            return;
        }
        self.worn.retain(|k| *k != key);
        let tex = self.texture(compositor, &face);
        self.free.extend(app.set_cart_face(key.0, key.1, tex));
        if let Some(interior) = interior {
            let tex = self.texture(compositor, &interior);
            self.free.extend(app.set_cart_interior(key.0, key.1, tex));
        }
    }
}

impl Drop for CartFaces {
    fn drop(&mut self) {
        let (lock, wake) = &*self.queue;
        lock.lock().unwrap_or_else(|e| e.into_inner()).closed = true;
        wake.notify_all();
    }
}

fn work(queue: &Shared, outbox: &Sender<Built>, root: &Path) {
    let (lock, wake) = &**queue;
    loop {
        let (key, cart, wear) = {
            let mut q = lock.lock().unwrap_or_else(|e| e.into_inner());
            loop {
                if q.closed {
                    return;
                }
                if let Some(job) = q.jobs.pop_front() {
                    break job;
                }
                q = wake.wait(q).unwrap_or_else(|e| e.into_inner());
            }
        };
        slot_store::header_index::verify(&cart.rom);
        let face = cart_face_worn(&cart, label_art(root, &cart), wear);
        let interior = shows_interior(&cart).then(|| cart_interior(&cart));
        if outbox.send((key, face, interior)).is_err() {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use slot_store::Platform;

    fn cart(stem: &str) -> Cart {
        Cart {
            platform: Platform::Gba,
            stem: stem.into(),
            rom: "a.gba".into(),
            label: None,
            title: stem.into(),
            code: String::new(),
            shell: None,
        }
    }

    #[test]
    fn a_face_built_from_an_old_cart_is_thrown_away() {
        let mut b = Building::default();
        let old = cart("A");
        let mut new = old.clone();
        new.label = Some("new.png".into());
        assert!(b.start((0, 0), &old, 0.0));
        assert!(!b.start((0, 0), &new, 0.0));
        assert!(!b.finish((0, 0), Some(&new)));
        assert!(b.start((0, 0), &new, 0.0));
        assert!(b.finish((0, 0), Some(&new)));
    }

    #[test]
    fn a_face_built_before_the_cart_wore_is_thrown_away() {
        let mut b = Building::default();
        assert!(b.start((0, 0), &cart("A"), 0.0));
        assert!(!b.start((0, 0), &cart("A"), 0.5));
        assert!(!b.finish((0, 0), Some(&cart("A"))));
        assert!(b.start((0, 0), &cart("A"), 0.5));
        assert!(b.finish((0, 0), Some(&cart("A"))));
    }

    #[test]
    fn the_same_cart_asked_twice_builds_once_and_lands() {
        let mut b = Building::default();
        assert!(b.start((0, 0), &cart("A"), 0.0));
        assert!(!b.start((0, 0), &cart("A"), 0.0));
        assert!(b.finish((0, 0), Some(&cart("A"))));
    }

    #[test]
    fn a_face_whose_seat_now_holds_another_cart_is_thrown_away() {
        let mut b = Building::default();
        assert!(b.start((3, 0), &cart("A"), 0.0));
        assert!(!b.finish((3, 0), Some(&cart("B"))));
        assert!(b.start((3, 1), &cart("A"), 0.0));
        assert!(!b.finish((3, 1), None));
    }
}
