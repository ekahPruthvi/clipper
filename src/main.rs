use gtk4::prelude::*;
use gtk4::{
    Application, ApplicationWindow, Box as GtkBox, Button, Label, ListBox, ListBoxRow,
    Orientation, ScrolledWindow, CssProvider, GestureClick, Picture, Frame, EventControllerKey, Image
};
use std::io::{Write, BufReader, Read};
use std::process::{Command, Stdio, exit};
use std::rc::Rc;
use std::cell::RefCell;

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
        listbox.set_selection_mode(gtk4::SelectionMode::Single);

        let head = Label::new(Some("clipper"));
        head.set_widget_name("head");
        head.set_justify(gtk4::Justification::Left);
        head.set_halign(gtk4::Align::Start);
        head.set_margin_start(20);

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
                        label.set_hexpand(true);
                        label.set_wrap_mode(gtk4::pango::WrapMode::WordChar);
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
        scroll.set_vscrollbar_policy(gtk4::PolicyType::Automatic);
        scroll.set_hscrollbar_policy(gtk4::PolicyType::Never);
        scroll.set_css_classes(&["scroller"]);
        listbox.set_css_classes(&["listbox"]);


        vbox.append(&head);
        vbox.append(&scroll);

        if listbox.first_child().is_none(){
            let image = Image::from_icon_name("clipper");
            let empty = Label::new(None);
            empty.set_markup("<b>Clipper is empty</b>\ncopy to show here");
            empty.set_widget_name("empty_message");
            empty.set_justify(gtk4::Justification::Center);
            image.set_pixel_size(86);
            image.set_css_classes(&["iconn"]);

            let empty_message_box = GtkBox::new(Orientation::Vertical, 10);
            empty_message_box.set_halign(gtk4::Align::Center);
            empty_message_box.set_valign(gtk4::Align::Center);


            empty_message_box.append(&image);
            empty_message_box.append(&empty);
            scroll.set_child(Some(&empty_message_box));
        } else {
            let wipe_button = Button::with_label("clear clipboard");
            wipe_button.add_css_class("wipe-button");
            let wipe_button_clone = wipe_button.clone();

            let confirming = Rc::new(RefCell::new(false));
            let confirming_clone = confirming.clone();

            wipe_button.connect_clicked(move |_| {
                let mut is_confirming = confirming_clone.borrow_mut();
                // First click → Change to "Confirm?" and update callback
                if *is_confirming {
                    // If we're already confirming, proceed to wipe
                    let _ = Command::new("cliphist").arg("wipe").output();
                    exit(0);
                } else {
                    // Enter confirm mode
                    *is_confirming = true;
                    wipe_button_clone.set_label("yes");
                    wipe_button_clone.add_css_class("confirming");

                    // Reset after 4 seconds
                    let wipe_button_reset = wipe_button_clone.clone();
                    let confirming_reset = confirming_clone.clone();
                    gtk4::glib::timeout_add_seconds_local(3, move || {
                        wipe_button_reset.set_label("clear clipboard");
                        wipe_button_reset.remove_css_class("confirming");
                        *confirming_reset.borrow_mut() = false;
                        gtk4::glib::ControlFlow::Break
                    });
                }
            });

            vbox.append(&wipe_button);
        }
        
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
            window {
                background: rgba(0, 0, 0, 0.38);
                border: 2px solid rgba(82, 82, 82, 0.29);
                border-radius: 20px;
            }

            .listbox {
                background-color:rgba(0, 0, 0, 0);
                border-radius: 20px;
            }

            .listbox > row{
                all:unset;
                margin: 5px;
                padding: 12px;
                border-radius: 8px;
                background-color:rgba(65, 65, 65, 0.25);
                min-height: 50px;
                font-weight: 300;
                font-size: 14px;
            }

            .listbox > row:hover {
                background-color:rgba(65, 65, 65, 0.38);
                border-bottom: 0.5px solid black;
            }

            .scroller{
                background: rgba(0, 0, 0, 0.15);
                border-radius: 15px;
                border: 2px solid rgba(82, 82, 82, 0.29);
                margin: 5px;
            }

            .scroller scrollbar {
                opacity: 0;
                min-width: 0;
                min-height: 0;
            } 

            button {
                all:unset;
                transform: scale(1.0);
                transition: background 0.2s ease, transform 0.2s ease;
                margin: 0px;
                padding: 16px;
                font-weight: bold;
            }

            button:hover {
                background: rgba(228, 228, 228, 0.1);
                transform: scale(1.1);
                font-weight: 800;
            }

            button.wipe-button {
                color: white;
                transition: background-color 200ms, transform 200ms, color 200ms;
            }

            button.wipe-button.confirming {
                background-color:rgb(248, 66, 66);
                color: white;
                transform: scale(1.05);
            }

            label {
                color: rgb(255, 255, 255);
                font-family : 'Cantarell'; 
            }

            #head {
                margin-top: 7px;
                font-size: 20px;
                font-weight: bold;
                color: rgba(255, 255, 255, 0.91);
            }      

            .iconn {
                color: rgba(255, 255, 255, 0.86);
            }

            #empty_message {
                color: rgba(255, 255, 255, 0.25);
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
