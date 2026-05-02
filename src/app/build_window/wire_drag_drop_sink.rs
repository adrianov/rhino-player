fn drain_input_stream_aggregate(
    stream: gio::InputStream,
    acc: Vec<u8>,
    done: Box<dyn FnOnce(Result<Vec<u8>, glib::Error>)>,
) {
    let chunk_size = 16_usize.saturating_mul(1024);
    let buf = vec![0u8; chunk_size];

    fn step(
        stream: gio::InputStream,
        mut acc: Vec<u8>,
        buf: Vec<u8>,
        done: Box<dyn FnOnce(Result<Vec<u8>, glib::Error>)>,
    ) {
        let s = stream.clone();
        s.read_async(
            buf,
            glib::Priority::default(),
            None::<&gio::Cancellable>,
            move |res| match res {
                Ok((b, got)) => {
                    if got == 0 {
                        drop(stream);
                        done(Ok(acc));
                        return;
                    }
                    acc.extend_from_slice(&b[..got]);
                    step(stream.clone(), acc, b, done);
                }
                Err((_b, err)) => {
                    drop(stream);
                    done(Err(err));
                }
            },
        );
    }

    step(stream, acc, buf, done);
}

fn finish_drop(drop: &gtk::gdk::Drop) {
    let acts = drop.actions();
    let copy = gtk::gdk::DragAction::COPY.intersection(acts);
    if !copy.is_empty() {
        drop.finish(copy);
        return;
    }
    drop.finish(if acts.is_empty() {
        gtk::gdk::DragAction::empty()
    } else {
        acts
    });
}
