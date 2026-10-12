use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use slot_store::Cart;
use slot_ui::{
    board_face, cart_face_worn, cart_interior, cover, gb_shell_of, label_size, padded, CartFace,
    TURN_PAD,
};

pub struct BuiltFaces {
    pub stem: String,
    pub wear: f32,
    pub interior: CartFace,
    pub board: CartFace,
    pub lid: CartFace,
}

pub struct FaceBuilder {
    requests: Sender<(Cart, f32)>,
    built: Receiver<BuiltFaces>,
}

impl FaceBuilder {
    pub fn spawn() -> Self {
        let (requests, inbox) = mpsc::channel::<(Cart, f32)>();
        let (outbox, built) = mpsc::channel();
        let spawned = thread::Builder::new()
            .name("slot-faces".into())
            .spawn(move || {
                while let Ok(mut job) = inbox.recv() {
                    while let Ok(newer) = inbox.try_recv() {
                        job = newer;
                    }
                    let (cart, wear) = job;
                    let (lw, lh) = label_size(&cart);
                    let art = cart.label.as_deref().and_then(|p| cover(p, lw, lh));
                    let faces = BuiltFaces {
                        stem: cart.stem.clone(),
                        wear,
                        interior: if gb_shell_of(&cart).is_none() {
                            cart_interior(&cart)
                        } else {
                            CartFace {
                                rgba: vec![0; 4],
                                w: 1,
                                h: 1,
                            }
                        },
                        board: board_face(&cart),
                        lid: padded(&cart_face_worn(&cart, art, wear), TURN_PAD),
                    };
                    if outbox.send(faces).is_err() {
                        return;
                    }
                }
            });
        if let Err(e) = spawned {
            eprintln!("slot: faces: worker thread failed to start: {e}");
        }
        FaceBuilder { requests, built }
    }

    pub fn request(&self, cart: Cart, wear: f32) {
        let _ = self.requests.send((cart, wear));
    }

    pub fn take(&self) -> Option<BuiltFaces> {
        let mut newest = None;
        while let Ok(faces) = self.built.try_recv() {
            newest = Some(faces);
        }
        newest
    }
}
