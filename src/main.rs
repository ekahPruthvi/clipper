use gtk4::prelude::*;
use gtk4::{
    Application, ApplicationWindow, Box as GtkBox, Button, Label, ListBox, ListBoxRow,
    Orientation, ScrolledWindow, CssProvider, MessageDialog, MessageType, GestureClick, Picture, Frame,
    ButtonsType, ResponseType, EventControllerKey,
};
use std::io::{Write, BufReader, Read};
use std::process::{Command, Stdio, exit};

fn main() {
    let app = Application::builder()
        .application_id("com.ekah.clipper")
        .build();

    app.connect_activate(|app| {
        let window = ApplicationWindow::builder()
            .application(app)
            .title("Clipper")
            .default_width(400)
            .default_height(500)
            .resizable(false)
            .build();

        let vbox = GtkBox::new(Orientation::Vertical, 5);
        let listbox = ListBox::new();
        listbox.set_selection_mode(gtk4::SelectionMode::None);

        if let Ok(output) = Command::new("cliphist").arg("list").output() {
            let entries = String::from_utf8_lossy(&output.stdout);
            for line in entries.lines() {
                let row = ListBoxRow::new();
                let hbox = GtkBox::new(Orientation::Horizontal, 5);

                let content = line.to_string();
                let decode = Command::new("cliphist")
                    .arg("decode")
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .spawn()
                    .expect("Failed to start cliphist decode");

                if let (Some(mut decode_stdin), Some(decode_stdout)) = (decode.stdin, decode.stdout) {
                    let _ = write!(decode_stdin, "{}", content);
                    drop(decode_stdin);

                    // Save to temp file
                    let tmp_path = "/tmp/clip-entry.bin";
                    let mut buffer = Vec::new();
                    let mut reader = BufReader::new(decode_stdout);
                    let _ = reader.read_to_end(&mut buffer);
                    std::fs::write(tmp_path, &buffer).expect("Failed to write to tmp file");

                    // Detect MIME
                    let mime = Command::new("file")
                        .arg("--mime-type")
                        .arg("-b")
                        .arg(tmp_path)
                        .output()
                        .ok()
                        .and_then(|o| String::from_utf8(o.stdout).ok())
                        .unwrap_or_default();

                    if mime.starts_with("image/") {
                        // Copy image to a predictable path
                        let _ = std::fs::copy(tmp_path, "/tmp/clipimg.png");
                        let frame = Frame::new(None);
                            frame.set_hexpand(true);
                            frame.set_height_request(200); // Crop height
                            frame.set_vexpand(false);
                        
                        let picture = Picture::for_filename(tmp_path);
                        picture.set_halign(gtk4::Align::Fill);

                        frame.set_child(Some(&picture));

                        hbox.append(&frame);
                    } else {
                        let label = Label::new(Some(&content));
                        label.set_wrap(true);
                        label.set_xalign(0.0);
                        hbox.append(&label);
                    }

                    row.set_child(Some(&hbox));

                    // Add copy on click
                    let content_clone = content.clone();
                    let gesture = GestureClick::new();
                    gesture.connect_pressed(move |_, _, _, _| {
                        let mut decode = Command::new("cliphist")
                            .arg("decode")
                            .stdin(Stdio::piped())
                            .stdout(Stdio::piped())
                            .spawn()
                            .expect("Failed to start cliphist decode");

                        if let Some(mut decode_stdin) = decode.stdin.take() {
                            let _ = write!(decode_stdin, "{}", content_clone);
                        }

                        if let Some(decode_stdout) = decode.stdout.take() {
                            let mut wlcopy = Command::new("wl-copy")
                                .stdin(Stdio::piped())
                                .spawn()
                                .expect("Failed to run wl-copy");

                            if let Some(mut wlcopy_stdin) = wlcopy.stdin.take() {
                                let _ = std::io::copy(&mut BufReader::new(decode_stdout), &mut wlcopy_stdin);
                            }
                        }
                    });
                    row.add_controller(gesture);
                    listbox.append(&row);
                }
            }
        }

        let scroll = ScrolledWindow::builder()
            .vexpand(true)
            .child(&listbox)
            .build();

        let wipe_button = Button::with_label("Wipe All");
        let window_clone = window.clone();
        wipe_button.connect_clicked(move |_| {
            let dialog = MessageDialog::builder()
                .transient_for(&window_clone)
                .modal(true)
                .message_type(MessageType::Question)
                .buttons(ButtonsType::YesNo)
                .text("Clear clipboard history?")
                .build();

            dialog.connect_response(move |d, resp| {
                if resp == ResponseType::Yes {
                    let _ = Command::new("cliphist").arg("wipe").output();
                    exit(0);
                }
                d.close();
            });

            dialog.show();
        });

        vbox.append(&scroll);
        vbox.append(&wipe_button);
        window.set_child(Some(&vbox));

        // Escape to exit
        let key_controller = EventControllerKey::new();
        let app_clone = app.clone();
        key_controller.connect_key_pressed(move |_, keyval, _, _| {
            if keyval == gtk4::gdk::Key::Escape {
                app_clone.quit();
            }
            gtk4::glib::Propagation::Stop
        });
        window.add_controller(key_controller);

        // Styling
        let css = CssProvider::new();
        css.load_from_data("
            button {
                margin: 6px;
                padding: 6px;
                font-weight: bold;
            }
            label {
                padding: 4px;
            }
        ");
        if let Some(display) = gtk4::gdk::Display::default() {
            gtk4::style_context_add_provider_for_display(
                &display,
                &css,
                gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
        }

        window.present();
    });

    app.run();
}
