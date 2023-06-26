
/*
* Dizzle is an alternative deezer client.
*/

use std::{thread::{spawn, JoinHandle}, sync::mpsc::{channel, Receiver, TryRecvError, Sender}, cell::RefCell, rc::Rc};

use serde_derive::Deserialize;
use eframe::{egui, epaint::{Color32, Vec2}, emath::Align2};

#[derive(Default, Deserialize)]
struct Config {
    pub(crate) info: rizzle::UserInfo,
}

fn main() {
     eframe::run_native(
        "Dizzle",
        eframe::NativeOptions::default(),
        Box::new(|creator| Box::new(App::init(creator)))
    ).expect("Cannot start Eframe App");
}

enum Ev {
    Starting,
    InvalidCredentials,
    LoggedOut,
    Ready,
    GotUser(rizzle::User),
}

enum Req {
    Quit,
    GetUser,
    Play(String),
}

struct SharedData {
    pub(crate) ev_recv: Receiver<Ev>,
    pub(crate) req_send: Sender<Req>,
    pub(crate) user_profile: Option<rizzle::User>,
}

struct App {
    lib_thread: Option<JoinHandle<()>>,
    shared: Rc<RefCell<SharedData>>,
    history: Vec<TabHistory>,
    selected: usize,
}

impl App {

    fn init(creator: &eframe::CreationContext<'_>) -> Self {

        let config_str = std::fs::read_to_string("Dizzle.toml").unwrap();
        let config: Config = toml::from_str(&config_str).unwrap();
        let mut info = config.info;
        info.user_agent = "Dizzle".to_string();

        let (req_send, req_recv) = channel();
        let (ev_send, ev_recv) = channel();
        let lib_ctx = creator.egui_ctx.clone();

        let lib_thread = spawn(move || {

            let send_event = move |ev| {
                ev_send.send(ev).unwrap();
                lib_ctx.request_repaint();
            };

            if info.arl.len() + info.sid.len() == 0 {
                send_event(Ev::LoggedOut);
            }

            send_event(Ev::Starting);

            let mut maybe_session = match rizzle::Session::new(info) {
                Ok(val) => {
                    Some(val)
                },
                Err(rizzle::Error::InvalidCredentials) => {
                    send_event(Ev::InvalidCredentials);
                    None
                },
                Err(other) => todo!("error: {}", other), // todo: handle network errors
            };

            if let Some(ref _session) = maybe_session {
                send_event(Ev::Ready);
            }

            loop {

                let req = req_recv.recv().unwrap();

                match maybe_session {
                    Some(ref mut session) => {
                        match req {
                            Req::Quit => break,
                            Req::GetUser => {
                                let user = session.user().unwrap();
                                send_event(Ev::GotUser(user));
                            },
                            Req::Play(name) => {
                                let result = session.search(&name).unwrap();
                                let stream = session.stream_raw(&result.tracks[0]).unwrap();
                                play_deezer_audio(stream).unwrap();
                            },
                            // _ => panic!("Invalid Req"),
                        }
                    },
                    None => {
                        match req {
                            Req::Quit => break,
                            _ => panic!("Invalid Req"),
                        }
                    },
                }

            }

        });

        let shared = Rc::new(RefCell::new(SharedData {
            req_send,
            ev_recv,
            user_profile: None,
        }));

        Self {
            history: vec![TabHistory::new(&shared, vec![Tab::new(&shared, TabKind::Empty)])],
            lib_thread: Some(lib_thread),
            shared,
            selected: 0,
        }

    }

    pub(crate) fn push_history(&mut self, kind: TabKind) {
        self.selected += 1;
        self.history.insert(self.selected, TabHistory::new(&self.shared, vec![Tab::new(&self.shared, kind)]));
    }

    fn handle_events(&mut self) {

        // process lib_thread events
        let maybe_event = match self.shared.borrow().ev_recv.try_recv() {
            Ok(val) => Some(val),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => unreachable!(),
        };

        if let Some(event) = maybe_event {
            match event {
                Ev::Starting => {
                    let mut starting_tab = Tab::new(&self.shared, TabKind::Starting);
                    starting_tab.push_item(ItemKind::Starting);
                    let mut starting_history = TabHistory::new(&self.shared, vec![starting_tab]);
                    self.history[self.selected] = starting_history;
                },
                Ev::InvalidCredentials => {
                    self.insert_and_switch_to_tab(TabKind::InvalidCredentials);
                },
                Ev::LoggedOut => {
                    self.insert_and_switch_to_tab(TabKind::LoggedOut);
                },
                Ev::Ready => {
                    // replace the "Starting" tab with the "Home" tab
                    let mut home_tab = Tab::new(&self.shared, TabKind::Home);
                    home_tab.push_item(ItemKind::UserProfile);
                    home_tab.push_item(ItemKind::Search);
                    home_tab.push_item(ItemKind::Recommended);
                    home_tab.push_item(ItemKind::News);
                    home_tab.push_item(ItemKind::Settings);
                    self.history[self.selected] = home_tab;
                    self.shared.borrow().req_send.send(Req::GetUser).unwrap();
                },
                Ev::GotUser(result) => {
                    self.shared.borrow_mut().user_profile = Some(result);
                }
            }
        }

    }

}

impl eframe::App for App {

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {

        self.handle_events();

        let mut needs_scroll = false;

        if !ctx.wants_keyboard_input() {

            let max_self_selected = (self.history.len() as isize - 1).clamp(0, self.history.len() as isize);
            let tab = &mut self.history[self.selected];
            let max_tab_selected = (tab.items.len() as isize - 1).clamp(0, tab.items.len() as isize);

            let mut tab_selected = tab.selected as isize;
            let mut self_selected = self.selected as isize;

            ctx.input(|input| {
                let shift = input.modifiers.shift_only();
                if input.key_pressed(egui::Key::J) && !shift { tab_selected = (tab_selected + 1).clamp(0, max_tab_selected) };
                if input.key_pressed(egui::Key::K) && !shift { tab_selected = (tab_selected - 1).clamp(0, max_tab_selected) };
                if input.key_pressed(egui::Key::J) &&  shift { self_selected = (self_selected + 1).clamp(0, max_self_selected) };
                if input.key_pressed(egui::Key::K) &&  shift { self_selected = (self_selected - 1).clamp(0, max_self_selected) };
                if input.key_pressed(egui::Key::Enter) { tab.go(tab.selected) }
            });

            if tab.selected != tab_selected as usize {
                needs_scroll = true;
            }

            tab.selected = tab_selected as usize;
            self.selected = self_selected as usize;
        }

        // draw the gui
        let frame = egui::Frame::default().fill(styles::BG2).inner_margin(10.0);
        egui::SidePanel::new(egui::panel::Side::Left, "Tabs").frame(frame).max_width(styles::TAB_PANEL_WIDTH).show(&ctx, |ui| {
            // tab group
            ui.vertical(|ui| {

                ui.style_mut().spacing.item_spacing.y = 7.0;

                for (idx, tab) in self.history.iter().enumerate() {

                    let ui_resp = ui.add(tab);

                    if ui_resp.clicked() {
                        self.selected = idx;
                    }

                    let color = if idx == self.selected { styles::PINK } else { styles::GREY };
                    ui.painter().rect_stroke(ui_resp.rect, 5.0, egui::Stroke::new(2.0, color));

                }

            });
        });

        let margin = egui::Margin { top: 10.0, bottom: 10.0, ..Default::default() };
        let frame = egui::Frame::default().fill(styles::BG1).inner_margin(margin);
        egui::CentralPanel::default().frame(frame).show(&ctx, |ui| {

            egui::ScrollArea::vertical().show(ui, |ui| {

                let margin = egui::Margin { left: 150.0, right: 150.0, ..Default::default() };
                let frame = egui::Frame::default().inner_margin(margin);
                frame.show(ui, |ui| {

                    let spacing = 13.0; // used later
                    ui.style_mut().spacing.item_spacing.y = spacing;

                    let tab = &mut self.history[self.selected];

                    let items = &tab.items;
                    let height: f32 = items.iter().map(|item| item.height() + spacing).sum();
                    let max_height = ui.max_rect().max.y;
                    let offset = (max_height / 2.0 - height / 2.0).clamp(0.0, height);
                    ui.add_space(offset);

                    for (idx, item) in items.iter().enumerate() {

                        let ui_resp = ui.add(item);

                        if ui_resp.clicked() {
                            tab.selected = idx;
                            needs_scroll = true;
                        }

                        if idx == tab.selected && needs_scroll {
                            ui_resp.scroll_to_me(None);
                        };

                        let color = if idx == tab.selected { styles::PINK } else { styles::GREY };
                        ui.painter().rect_stroke(ui_resp.rect, 5.0, egui::Stroke::new(2.0, color));

                    };

                });
            });
        });

    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.shared.borrow().req_send.send(Req::Quit).unwrap();
        self.lib_thread.take().unwrap().join().unwrap();
    }

}

struct TabHistory {
    shared: Rc<RefCell<SharedData>>,
    tabs: Vec<Tab>,
    selected: usize,
}

impl TabHistory {
    
    pub(crate) fn new(shared: &Rc<RefCell<SharedData>>, tabs: Vec<Tab>) -> Self {
        Self {
            shared: Rc::clone(shared),
            tabs,
            selected: 0,
        }
    }
    
}

#[derive(Clone)]
enum TabKind {
    Empty,
    InvalidCredentials,
    LoggedOut,
    Starting,
    Home,
}

#[derive(Clone)]
struct Tab {
    pub(crate) shared: Rc<RefCell<SharedData>>,
    pub(crate) kind: TabKind,
    pub(crate) items: Vec<Item>,
    pub(crate) selected: usize,
}

impl Tab {

    pub(crate) fn new(shared: &Rc<RefCell<SharedData>>, kind: TabKind) -> Self {
        Self { shared: Rc::clone(shared), kind, items: Vec::new(), selected: 0 }
    }

    pub(crate) fn name(&self) -> &'static str { // todo: remove this
        match self.kind {
            TabKind::Empty => "",
            TabKind::InvalidCredentials => "Invalid Credentials",
            TabKind::LoggedOut => "Logged Out",
            TabKind::Starting => "Starting",
            TabKind::Home => "Home",
        }
    }

    pub(crate) fn push_item(&mut self, kind: ItemKind) {
        self.items.push(Item::new(&self.shared, kind));
    }

    pub(crate) fn go(&mut self, index: usize) {
        
        todo!()

    }

}

impl egui::Widget for &Tab {

    fn ui(self, ui: &mut egui::Ui) -> egui::Response {

        let max_rect = ui.max_rect();
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(max_rect.width(), styles::TAB_HEIGHT), egui::Sense::click());
        let painter = ui.painter_at(rect);
        painter.text(rect.center(), Align2::CENTER_CENTER, &self.name(), egui::FontId::proportional(20.0), Color32::WHITE);
        resp

    }

}

#[derive(Clone)]
enum ItemKind {
    Starting,
    UserProfile,
    Search,
    Recommended,
    News,
    Settings,
}

#[derive(Clone)]
struct Item {
    pub(crate) shared: Rc<RefCell<SharedData>>,
    kind: ItemKind,
}

impl Item {

    fn new(shared: &Rc<RefCell<SharedData>>, kind: ItemKind) -> Self {
        Self { shared: Rc::clone(shared), kind }
    }

    fn height(&self) -> f32 {
        match self.kind {
            ItemKind::Starting => 400.0,
            ItemKind::UserProfile => 200.0,
            ItemKind::Search => 100.0,
            ItemKind::Recommended => 300.0,
            ItemKind::News => 200.0,
            ItemKind::Settings => 200.0,
        }
    }

}

impl egui::Widget for &Item {

    fn ui(self, ui: &mut egui::Ui) -> egui::Response {

        let width = ui.max_rect().width();
        let height = self.height();

        match self.kind {
            ItemKind::Starting => {
                let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, height), egui::Sense::click());
                let painter = ui.painter_at(rect);
                let mut pos = rect.center();
                pos.y -= 35.0 / 2.0;
                painter.text(pos, Align2::CENTER_CENTER, "Starting", egui::FontId::proportional(30.0), Color32::WHITE);
                pos.y += 35.0 / 2.0;
                pos.y += 15.0;
                painter.text(pos, Align2::CENTER_CENTER, "Loading", egui::FontId::proportional(15.0), Color32::WHITE);
                resp
            },
            ItemKind::UserProfile => {
                let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, height), egui::Sense::click());
                let painter = ui.painter_at(rect);
                let pos = rect.center();
                painter.text(pos, Align2::CENTER_CENTER, "User Profile", egui::FontId::proportional(30.0), Color32::WHITE);
                resp
            },
            ItemKind::Search => {
                let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, height), egui::Sense::click());
                let painter = ui.painter_at(rect);
                let pos = rect.center();
                painter.text(pos, Align2::CENTER_CENTER, "Search", egui::FontId::proportional(30.0), Color32::WHITE);
                resp
            },
            ItemKind::Recommended => {
                let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, height), egui::Sense::click());
                let painter = ui.painter_at(rect);
                let pos = rect.center();
                painter.text(pos, Align2::CENTER_CENTER, "Recommended", egui::FontId::proportional(30.0), Color32::WHITE);
                resp
            },
            ItemKind::News => {
                let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, height), egui::Sense::click());
                let painter = ui.painter_at(rect);
                let pos = rect.center();
                painter.text(pos, Align2::CENTER_CENTER, "News", egui::FontId::proportional(30.0), Color32::WHITE);
                resp
            },
            ItemKind::Settings => {
                let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, height), egui::Sense::click());
                let painter = ui.painter_at(rect);
                let pos = rect.center();
                painter.text(pos, Align2::CENTER_CENTER, "Settings", egui::FontId::proportional(30.0), Color32::WHITE);
                resp
            },
        }

    }

}

fn play_deezer_audio(audio: rizzle::RawStream) -> anyhow::Result<()> {

    let pcm = alsa::PCM::new("default", alsa::Direction::Playback, false)?;

    let params = alsa::pcm::HwParams::any(&pcm)?;
    params.set_channels(2)?;
    params.set_rate(44100, alsa::ValueOr::Nearest)?;
    params.set_format(alsa::pcm::Format::s16())?;
    params.set_access(alsa::pcm::Access::RWInterleaved)?;

    pcm.hw_params(&params)?;
    let alsa_io = pcm.io_checked::<i16>()?;

    let hwp = pcm.hw_params_current().unwrap();
    let swp = pcm.sw_params_current().unwrap();
    swp.set_start_threshold(hwp.get_buffer_size().unwrap()).unwrap();
    pcm.sw_params(&swp).unwrap();

    for packet in audio {
        alsa_io.writei(&packet?)?;
    }

    pcm.drain()?;

    Ok(())

}

mod styles {
    use eframe::egui::Color32;

    pub(crate) const TAB_PANEL_WIDTH:  f32 = 160.0;
    pub(crate) const TAB_HEIGHT: f32 = 50.0;

    pub(crate) const BG1:  Color32 = Color32::from_rgb(0x12, 0x12, 0x16);
    pub(crate) const BG2:  Color32 = Color32::from_rgb(0x19, 0x19, 0x22);
    pub(crate) const PINK: Color32 = Color32::from_rgb(0xF0, 0x53, 0x66);
    pub(crate) const GREY: Color32 = Color32::from_rgb(0x32, 0x32, 0x3D);

}

