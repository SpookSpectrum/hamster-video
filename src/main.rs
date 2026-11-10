use eframe::egui::{self, Color32, RichText};
use serde::{Deserialize, Serialize};
mod engine;
use engine::{progress, stop, valid_url, worker, MediaInfo, Running};
use std::{
    collections::VecDeque,
    path::PathBuf,
    process::Command,
    sync::mpsc,
    time::{Duration, Instant},
};

const ACCENT: Color32 = Color32::from_rgb(69, 215, 183);
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
enum Appearance {
    Dark,
    Light,
    #[default]
    Automatic,
}
impl Appearance {
    fn next(self) -> Self {
        match self {
            Self::Automatic => Self::Light,
            Self::Light => Self::Dark,
            Self::Dark => Self::Automatic,
        }
    }
    fn label(self) -> &'static str {
        match self {
            Self::Dark => "Dark",
            Self::Light => "Light",
            Self::Automatic => "Automatic",
        }
    }
    fn preference(self) -> egui::ThemePreference {
        match self {
            Self::Dark => egui::ThemePreference::Dark,
            Self::Light => egui::ThemePreference::Light,
            Self::Automatic => egui::ThemePreference::System,
        }
    }
}
fn configure_styles(ctx: &egui::Context) {
    for theme in [egui::Theme::Dark, egui::Theme::Light] {
        ctx.style_mut_of(theme, |style| {
            style.spacing.item_spacing = egui::vec2(12., 12.);
            style.spacing.button_padding = egui::vec2(16., 10.);
            if theme == egui::Theme::Dark {
                style.visuals.panel_fill = Color32::from_rgb(15, 23, 32);
                style.visuals.window_fill = Color32::from_rgb(23, 33, 44);
                style.visuals.selection.bg_fill = Color32::from_rgb(25, 104, 94);
            } else {
                style.visuals.panel_fill = Color32::from_rgb(246, 248, 249);
                style.visuals.window_fill = Color32::WHITE;
                style.visuals.selection.bg_fill = Color32::from_rgb(176, 232, 221);
            }
        });
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
enum Compatibility {
    #[default]
    Apple,
    Original,
    Hevc,
}
impl Compatibility {
    fn slug(self) -> &'static str {
        match self {
            Self::Apple => "h264",
            Self::Original => "original",
            Self::Hevc => "hevc",
        }
    }
    fn short(self) -> &'static str {
        match self {
            Self::Apple => "H.264",
            Self::Original => "Original",
            Self::Hevc => "HEVC",
        }
    }
}
fn appearance_button(ui: &mut egui::Ui, mode: Appearance) -> bool {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(32., 32.), egui::Sense::click());
    let response = response.on_hover_text(format!(
        "Appearance: {}. Click for {}.",
        mode.label(),
        mode.next().label()
    ));
    response.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::WidgetType::Button,
            true,
            format!("Appearance: {}", mode.label()),
        )
    });
    let color = ui.style().interact(&response).fg_stroke.color;
    let stroke = egui::Stroke::new(1.5_f32, color);
    let c = rect.center();
    let painter = ui.painter();
    if response.hovered() || response.has_focus() {
        painter.rect_filled(rect, 5., ui.visuals().widgets.hovered.bg_fill);
    }
    match mode {
        Appearance::Automatic => {
            let screen = egui::Rect::from_center_size(c - egui::vec2(0., 2.), egui::vec2(20., 13.));
            painter.rect_stroke(screen, 2., stroke, egui::StrokeKind::Inside);
            painter.line_segment([c + egui::vec2(0., 5.), c + egui::vec2(0., 9.)], stroke);
            painter.line_segment([c + egui::vec2(-5., 9.), c + egui::vec2(5., 9.)], stroke);
        }
        Appearance::Light => {
            painter.circle_stroke(c, 4., stroke);
            for i in 0..8 {
                let a = i as f32 * std::f32::consts::TAU / 8.;
                let v = egui::vec2(a.cos(), a.sin());
                painter.line_segment([c + v * 7., c + v * 10.], stroke);
            }
        }
        Appearance::Dark => {
            let mut points = Vec::new();
            for i in 0..=24 {
                let a = std::f32::consts::FRAC_PI_2 + (i as f32 / 24.) * std::f32::consts::PI;
                points.push(c + egui::vec2(a.cos() * 8., a.sin() * 8.));
            }
            painter.add(egui::Shape::line(points, stroke));
            let mut points = Vec::new();
            for i in 0..=24 {
                let a = -std::f32::consts::FRAC_PI_2 - (i as f32 / 24.) * std::f32::consts::PI;
                points.push(c + egui::vec2(a.cos() * 4., a.sin() * 8.));
            }
            painter.add(egui::Shape::line(points, stroke));
        }
    }
    response.clicked()
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
struct Settings {
    appearance: Appearance,
    compatibility: Compatibility,
    audio_language: Option<String>,
    folder: String,
    quality: u32,
    audio: bool,
    playlist: bool,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            appearance: Appearance::Automatic,
            compatibility: Compatibility::Apple,
            audio_language: None,
            folder: format!(
                "{}/Downloads/Hamster Video",
                std::env::var("HOME").unwrap_or_default()
            ),
            quality: 2160,
            audio: false,
            playlist: false,
        }
    }
}
#[derive(Clone, Serialize, Deserialize)]
struct Job {
    #[serde(default = "engine::new_job_id")]
    id: String,
    #[serde(default)]
    title: String,
    url: String,
    settings: Settings,
    status: String,
    progress: f32,
    detail: String,
}
#[derive(Default, Serialize, Deserialize)]
struct Saved {
    settings: Settings,
    jobs: Vec<Job>,
}
enum Event {
    Line(usize, String),
    Done(usize, bool, bool),
    Stage(usize, String),
    Encoding(usize, f32, String),
    Metadata(usize, String, Result<MediaInfo, String>),
}
struct Hamster {
    icon: egui::TextureHandle,
    saved: Saved,
    input: String,
    message: String,
    queue: VecDeque<usize>,
    running: Option<Running>,
    tx: mpsc::Sender<Event>,
    rx: mpsc::Receiver<Event>,
    log: VecDeque<String>,
    show_log: bool,
    catalog: Option<MediaInfo>,
    probe_generation: usize,
    observed_url: String,
    probe_job: Option<Running>,
    probe_due: Option<Instant>,
    track_message: String,
}
fn state_path() -> PathBuf {
    if let Ok(p) = std::env::var("HAMSTER_STATE") {
        return p.into();
    }
    PathBuf::from(std::env::var("HOME").unwrap_or_default())
        .join("Library/Application Support/Hamster Video/state.json")
}
impl Hamster {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        configure_styles(&cc.egui_ctx);
        let data = eframe::icon_data::from_png_bytes(include_bytes!("../assets/hamster-icon.png"))
            .expect("bundled icon");
        let icon = cc.egui_ctx.load_texture(
            "hamster",
            egui::ColorImage::from_rgba_unmultiplied(
                [data.width as usize, data.height as usize],
                &data.rgba,
            ),
            egui::TextureOptions::LINEAR,
        );
        let mut saved: Saved = std::fs::read(state_path())
            .or_else(|_| {
                std::fs::read(
                    PathBuf::from(std::env::var("HOME").unwrap_or_default())
                        .join("Library/Application Support/Harbor Video/state.json"),
                )
            })
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        for j in &mut saved.jobs {
            if j.status == "Cancelling" {
                match engine::cleanup(j) {
                    Ok(()) => j.status = "Cancelled".into(),
                    Err(e) => {
                        j.status = "Failed".into();
                        j.detail = e;
                    }
                }
            }
            if ["Downloading", "Converting", "Queued"].contains(&j.status.as_str()) {
                j.status = "Paused".into();
                j.detail = "Interrupted last session. Resume to continue partial downloads.".into();
            }
        }
        cc.egui_ctx
            .set_theme(saved.settings.appearance.preference());
        let (tx, rx) = mpsc::channel();
        Self {
            icon,
            saved,
            input: String::new(),
            message: String::new(),
            queue: VecDeque::new(),
            running: None,
            tx,
            rx,
            log: VecDeque::new(),
            show_log: false,
            catalog: None,
            probe_generation: 0,
            observed_url: String::new(),
            probe_job: None,
            probe_due: None,
            track_message: String::new(),
        }
    }
    fn persist(&self) {
        let p = state_path();
        if let Some(d) = p.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        if let Ok(s) = serde_json::to_vec_pretty(&self.saved) {
            let tmp = p.with_extension("tmp");
            if std::fs::write(&tmp, s).is_ok() {
                let _ = std::fs::rename(tmp, p);
            }
        }
    }
    fn add(&mut self) {
        self.inspect_input();
        let links: Vec<_> = self
            .input
            .lines()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect();
        if links.is_empty() {
            self.message = "Paste a video link first.".into();
            return;
        }
        if links.iter().any(|s| !valid_url(s)) {
            self.message = "Use a complete http:// or https:// link, one per line.".into();
            return;
        }
        if self.saved.settings.folder.trim().is_empty()
            || !PathBuf::from(&self.saved.settings.folder).is_absolute()
        {
            self.message = "Choose an absolute destination folder.".into();
            return;
        }
        let mut settings = self.saved.settings.clone();
        // Original is explicit when the site identifies it; otherwise retain its default.
        if settings.audio_language.is_none() {
            settings.audio_language = self
                .catalog
                .as_ref()
                .and_then(|m| m.tracks.iter().find(|t| t.original))
                .map(|t| t.code.clone());
        }
        for u in links {
            let i = self.saved.jobs.len();
            self.saved.jobs.push(Job {
                id: engine::new_job_id(),
                title: String::new(),
                url: u,
                settings: settings.clone(),
                status: "Queued".into(),
                progress: 0.,
                detail: "Waiting to download".into(),
            });
            self.queue.push_back(i);
        }
        self.input.clear();
        self.message.clear();
        self.persist();
    }
    fn inspect_input(&mut self) {
        let url = self
            .input
            .lines()
            .map(str::trim)
            .find(|l| !l.is_empty())
            .unwrap_or("")
            .to_string();
        if url != self.observed_url {
            if let Some(r) = self.probe_job.take() {
                stop(&r);
            }
            self.probe_generation = self.probe_generation.wrapping_add(1);
            self.observed_url = url.clone();
            self.catalog = None;
            self.track_message.clear();
            self.saved.settings.audio_language = None;
            self.probe_due = valid_url(&url).then(|| Instant::now() + Duration::from_millis(700));
        }
        if self.probe_due.is_some_and(|due| Instant::now() >= due) {
            self.probe_due = None;
            self.start_probe();
        }
    }
    fn start_probe(&mut self) {
        if !valid_url(&self.observed_url) {
            return;
        }
        if let Some(r) = self.probe_job.take() {
            stop(&r);
        }
        self.probe_generation = self.probe_generation.wrapping_add(1);
        let r = Running::new(self.probe_generation);
        engine::probe(self.observed_url.clone(), r.clone(), self.tx.clone());
        self.probe_job = Some(r);
        self.track_message = "Checking available audio tracks…".into();
    }
    fn tick(&mut self) {
        self.inspect_input();
        while let Ok(e) = self.rx.try_recv() {
            match e {
                Event::Metadata(generation, url, result) => {
                    if generation != self.probe_generation || url != self.observed_url {
                        continue;
                    }
                    self.probe_job = None;
                    match result {
                        Ok(info) => {
                            self.track_message = if info.tracks.is_empty() {
                                "The site did not list alternate audio languages.".into()
                            } else {
                                format!("{} available audio language(s)", info.tracks.len())
                            };
                            self.catalog = Some(info);
                        }
                        Err(e) => {
                            self.track_message="Could not check audio tracks. You can retry or use the site default.".into();
                            self.log.push_back(e);
                        }
                    }
                }
                Event::Encoding(i, p, detail) => {
                    if let Some(j) = self.saved.jobs.get_mut(i) {
                        if j.status != "Cancelling" {
                            j.progress = p;
                            j.detail = detail;
                        }
                    }
                }
                Event::Stage(i, detail) => {
                    if let Some(j) = self.saved.jobs.get_mut(i) {
                        if j.status == "Cancelling" {
                            continue;
                        }
                        j.progress = 0.;
                        j.status = "Converting".into();
                        j.detail = detail;
                    }
                }
                Event::Line(i, line) => {
                    if self.log.len() > 300 {
                        self.log.pop_front();
                    }
                    self.log.push_back(line.clone());
                    if let Some(j) = self.saved.jobs.get_mut(i) {
                        if let Some((p, d)) = progress(&line) {
                            j.progress = p;
                            j.detail = d;
                        } else if let Some(title) = line.strip_prefix("TITLE|") {
                            j.title = title.into();
                        } else if let Some(file) = line.strip_prefix("SAVED|") {
                            j.detail = file.into();
                        } else if line.contains("ERROR:") {
                            j.detail = line;
                        }
                    }
                }
                Event::Done(i, ok, cancelled) => {
                    if let Some(j) = self.saved.jobs.get_mut(i) {
                        j.status = if cancelled
                            && self.running.as_ref().is_some_and(|r| {
                                r.discard.load(std::sync::atomic::Ordering::SeqCst)
                            }) {
                            "Cancelled"
                        } else if cancelled {
                            "Paused"
                        } else if ok {
                            "Complete"
                        } else {
                            "Failed"
                        }
                        .into();
                        if ok {
                            j.progress = 1.;
                        }
                        if cancelled {
                            j.detail = "Paused. Resume to continue available partial files.".into();
                        }
                    }
                    self.running = None;
                    self.persist();
                }
            }
        }
        if self.running.is_none() && self.queue.is_empty() {
            self.saved.jobs.retain(|j| j.status != "Cancelled");
        }
        if self.running.is_none() {
            if let Some(i) = self.queue.pop_front() {
                self.saved.jobs[i].status = "Downloading".into();
                let r = Running::new(i);
                worker(i, self.saved.jobs[i].clone(), self.tx.clone(), r.clone());
                self.running = Some(r);
                self.persist();
            }
        }
    }
}
impl Drop for Hamster {
    fn drop(&mut self) {
        if let Some(r) = &self.running {
            stop(r);
        }
        if let Some(r) = &self.probe_job {
            stop(r);
        }
        self.persist();
    }
}
impl eframe::App for Hamster {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::Q)) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        self.tick();
        ctx.set_theme(self.saved.settings.appearance.preference());
        let accent = if ctx.theme() == egui::Theme::Dark {
            ACCENT
        } else {
            Color32::from_rgb(0, 108, 88)
        };
        let muted = if ctx.theme() == egui::Theme::Dark {
            Color32::from_rgb(153, 171, 188)
        } else {
            Color32::from_rgb(86, 102, 116)
        };
        ctx.request_repaint_after(Duration::from_millis(120));
        egui::SidePanel::left("sidebar")
            .exact_width(190.)
            .resizable(false)
            .show(ctx, |ui| {
                ui.add_space(20.);
                ui.image((self.icon.id(), egui::vec2(82., 82.)));
                ui.label(RichText::new("HAMSTER").size(27.).strong().color(accent));
                ui.label(RichText::new("V I D E O").size(13.).color(muted));
                ui.add_space(36.);
                ui.label(RichText::new("A home for your videos.").size(16.));
                ui.add_space(20.);
                ui.separator();
                ui.label(format!(
                    "{} downloads",
                    self.saved
                        .jobs
                        .iter()
                        .filter(|j| j.status != "Cancelled")
                        .count()
                ));
                ui.label(format!(
                    "{} complete",
                    self.saved
                        .jobs
                        .iter()
                        .filter(|j| j.status == "Complete")
                        .count()
                ));
                ui.label(format!("{} waiting", self.queue.len()));
                ui.add_space(24.);
                if ui.button("Open destination").clicked() {
                    let _ = std::fs::create_dir_all(&self.saved.settings.folder);
                    let _ = Command::new("/usr/bin/open")
                        .arg(&self.saved.settings.folder)
                        .spawn();
                }
                ui.checkbox(&mut self.show_log, "Engine log");
                ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                    ui.add_space(18.);
                    ui.label(
                        RichText::new("Hamster Video 0.2.2\nUniversal Mac app")
                            .small()
                            .color(muted),
                    );
                });
            });
        egui::CentralPanel::default().show(ctx,|ui|{
   ui.add_space(12.);
   ui.horizontal(|ui|{ui.heading(RichText::new("Bring your videos home").size(28.));ui.with_layout(egui::Layout::right_to_left(egui::Align::Center),|ui|{if appearance_button(ui,self.saved.settings.appearance){self.saved.settings.appearance=self.saved.settings.appearance.next();ctx.set_theme(self.saved.settings.appearance.preference());self.persist();}});});ui.label(RichText::new("Paste a link. Choose your quality. Keep watching offline.").color(muted));ui.add_space(16.);
   egui::Frame::group(ui.style()).inner_margin(18.).show(ui,|ui|{
    ui.horizontal(|ui|{ui.label(RichText::new("NEW DOWNLOAD").strong().color(accent));if ui.small_button("Paste clipboard").clicked(){if let Ok(o)=Command::new("/usr/bin/pbpaste").output(){self.input=String::from_utf8_lossy(&o.stdout).trim().into();}}});
    ui.add(egui::TextEdit::multiline(&mut self.input).hint_text("https://…\nOne video or playlist link per line").desired_rows(2).desired_width(f32::INFINITY));
    ui.horizontal_wrapped(|ui|{
     ui.label("Save as");egui::ComboBox::from_id_salt("format").selected_text(if self.saved.settings.audio{"MP3 audio"}else{"Video"}).show_ui(ui,|ui|{ui.selectable_value(&mut self.saved.settings.audio,false,"Video");ui.selectable_value(&mut self.saved.settings.audio,true,"MP3 audio");});
     ui.add_enabled_ui(!self.saved.settings.audio,|ui|{egui::ComboBox::from_id_salt("quality").selected_text(format!("Up to {}p",self.saved.settings.quality)).show_ui(ui,|ui|{for (n,s) in [(2160,"4K · 2160p"),(1440,"1440p"),(1080,"1080p"),(720,"720p"),(480,"480p")]{ui.selectable_value(&mut self.saved.settings.quality,n,s);}});});
     ui.checkbox(&mut self.saved.settings.playlist,"Whole playlist");
    });
    if !self.saved.settings.audio {
        ui.label(RichText::new("Video compatibility").strong());
        ui.vertical(|ui|{
            ui.spacing_mut().item_spacing.y=5.;
            ui.radio_value(&mut self.saved.settings.compatibility,Compatibility::Apple,"Apple compatible (H.264 / MP4) — Recommended");
            ui.radio_value(&mut self.saved.settings.compatibility,Compatibility::Original,"Original quality — Keep the source codec");
            ui.radio_value(&mut self.saved.settings.compatibility,Compatibility::Hevc,"High efficiency (HEVC / H.265) — Size depends on the source");
        });
    }
    ui.horizontal_wrapped(|ui|{
        ui.label("Audio language");
        let selected=self.saved.settings.audio_language.as_deref().map(engine::language_name).unwrap_or_else(||"Original / site default".into());
        egui::ComboBox::from_id_salt("audio-language").width(225.).selected_text(selected).show_ui(ui,|ui|{
            ui.selectable_value(&mut self.saved.settings.audio_language,None,"Original / site default");
            if let Some(info)=&self.catalog {for track in &info.tracks {ui.selectable_value(&mut self.saved.settings.audio_language,Some(track.code.clone()),format!("{}{}",track.label,if track.original{" · Original"}else{""}));}}
        });
        if self.probe_job.is_some(){ui.spinner();}else if ui.add_enabled(valid_url(&self.observed_url),egui::Button::new("Check tracks").small()).clicked(){self.start_probe();}
    });
    if !self.track_message.is_empty(){ui.label(RichText::new(&self.track_message).small().color(muted));}
    if self.input.lines().filter(|l|!l.trim().is_empty()).count()>1 || self.saved.settings.playlist {ui.label(RichText::new("Tracks shown for the first video. Your chosen language must be available in each video.").small().color(muted));}
    ui.horizontal(|ui|{ui.label("Folder");ui.add(egui::TextEdit::singleline(&mut self.saved.settings.folder).desired_width((ui.available_width()-100.).max(100.)));if ui.button("Choose…").clicked(){if let Ok(o)=Command::new("/usr/bin/osascript").args(["-e","POSIX path of (choose folder with prompt \"Save Hamster downloads to\")"]).output(){if o.status.success(){self.saved.settings.folder=String::from_utf8_lossy(&o.stdout).trim().into();self.persist();}}}});
    ui.horizontal(|ui|{if ui.add(egui::Button::new(RichText::new("Add to downloads").strong().color(Color32::from_rgb(10,30,26))).fill(ACCENT)).clicked(){self.add();}ui.label(RichText::new("Quality depends on the source.").small().color(muted));});
    if !self.message.is_empty(){ui.colored_label(Color32::from_rgb(255,180,110),&self.message);}
   });
   ui.add_space(12.);ui.horizontal(|ui|{ui.heading("Downloads");if ui.small_button("Clear finished").clicked() && self.running.is_none() && self.queue.is_empty(){self.saved.jobs.retain(|j|j.status!="Complete");self.persist();}});
   if self.show_log {egui::CollapsingHeader::new("Download engine output").default_open(true).show(ui,|ui|{egui::ScrollArea::vertical().id_salt("logs").max_height(130.).stick_to_bottom(true).show(ui,|ui|{for l in &self.log{ui.monospace(l);}});});}
   let mut resume=None;let mut pause=None;let mut cancel=None;
   egui::ScrollArea::vertical().id_salt("jobs").show(ui,|ui|{
    if self.saved.jobs.is_empty(){ui.add_space(35.);ui.vertical_centered(|ui|{ui.label(RichText::new("Your hamster is ready.").size(23.).color(muted));ui.label("Paste your first link above to begin.");});}
    for (i,j) in self.saved.jobs.iter().enumerate().rev(){if j.status=="Cancelled"{continue;}egui::Frame::group(ui.style()).inner_margin(14.).show(ui,|ui|{
     ui.horizontal(|ui|{ui.label(RichText::new(&j.status).strong().color(if j.status=="Failed"{Color32::from_rgb(255,145,130)}else{ACCENT}));ui.label(if j.settings.audio{"MP3".into()}else{format!("≤ {}p",j.settings.quality)});ui.with_layout(egui::Layout::right_to_left(egui::Align::Center),|ui|{if ["Paused","Failed"].contains(&j.status.as_str()){if ui.small_button("Resume / retry").clicked(){resume=Some(i);}}if ["Downloading","Converting","Queued"].contains(&j.status.as_str()) && ui.small_button("Pause").clicked(){pause=Some(i);}if ["Downloading","Converting","Queued","Paused","Failed"].contains(&j.status.as_str()) && ui.small_button("Cancel").on_hover_text("Stop this download and remove its unfinished files").clicked(){cancel=Some(i);}if j.status=="Complete" && ui.small_button("Show folder").clicked(){let _=Command::new("/usr/bin/open").arg(&j.settings.folder).spawn();}});});
     if !j.title.is_empty(){ui.label(RichText::new(&j.title).size(16.).strong());}ui.add(egui::Label::new(RichText::new(&j.url).size(14.)).truncate());if j.status=="Converting"{ui.spinner();}ui.add(egui::ProgressBar::new(j.progress).fill(ACCENT).show_percentage().animate(j.status=="Downloading"));ui.add(egui::Label::new(RichText::new(&j.detail).small().color(muted)).wrap());
    });}
   });
   if let Some(i)=cancel {
       if let Some(r)=self.running.as_ref().filter(|r|r.index==i) {engine::cancel(r);self.saved.jobs[i].status="Cancelling".into();self.saved.jobs[i].detail="Stopping and removing unfinished files…".into();}
       else {self.queue.retain(|n|*n!=i);match engine::cleanup(&self.saved.jobs[i]) {Ok(())=>self.saved.jobs[i].status="Cancelled".into(),Err(e)=>{self.saved.jobs[i].status="Failed".into();self.saved.jobs[i].detail=e;}}}
       self.persist();
   }
   if let Some(i)=resume{self.saved.jobs[i].status="Queued".into();self.queue.push_back(i);self.persist();}
   if let Some(i)=pause{if let Some(r)=&self.running {if r.index==i{stop(r);}}if self.queue.contains(&i){self.queue.retain(|n|*n!=i);self.saved.jobs[i].status="Paused".into();self.persist();}}
  });
    }
}
fn main() -> eframe::Result {
    let cli: Vec<String> = std::env::args().collect();
    if cli.get(1).map(String::as_str) == Some("--check-download") {
        if cli.len() < 4 || !valid_url(&cli[2]) {
            eprintln!("Usage: hamster-video --check-download URL ABSOLUTE_FOLDER [--audio]");
            std::process::exit(2);
        }
        let mut settings = Settings::default();
        settings.folder = cli[3].clone();
        settings.audio = cli.iter().any(|s| s == "--audio");
        if let Some(q) = cli.iter().find_map(|s| s.strip_prefix("--quality=")) {
            settings.quality = q.parse().unwrap_or(2160);
        }
        settings.compatibility = match cli.iter().find_map(|s| s.strip_prefix("--compatibility=")) {
            Some("original") => Compatibility::Original,
            Some("hevc") => Compatibility::Hevc,
            _ => Compatibility::Apple,
        };
        settings.audio_language = cli
            .iter()
            .find_map(|s| s.strip_prefix("--language="))
            .map(String::from);
        let job = Job {
            id: engine::new_job_id(),
            title: String::new(),
            url: cli[2].clone(),
            settings,
            status: String::new(),
            progress: 0.,
            detail: String::new(),
        };
        let (tx, rx) = mpsc::channel();
        let running = Running::new(0);
        worker(0, job, tx, running.clone());
        loop {
            match rx.recv_timeout(Duration::from_secs(180)) {
                Ok(Event::Line(_, s)) => println!("{s}"),
                Ok(Event::Done(_, ok, _)) => std::process::exit(if ok { 0 } else { 1 }),
                Ok(Event::Stage(_, s)) => println!("{s}"),
                Ok(Event::Encoding(_, p, s)) => println!("CONVERT|{p}|{s}"),
                Ok(Event::Metadata(_, _, _)) => {}
                Err(_) => {
                    stop(&running);
                    std::process::exit(3)
                }
            }
        }
    }
    if cli.get(1).map(String::as_str) == Some("--check-tracks") {
        let url = cli
            .get(2)
            .filter(|u| valid_url(u))
            .expect("Supply an HTTP(S) video URL");
        let (tx, rx) = mpsc::channel();
        let r = Running::new(0);
        engine::probe(url.clone(), r.clone(), tx);
        match rx.recv_timeout(Duration::from_secs(100)) {
            Ok(Event::Metadata(_, _, Ok(info))) => {
                println!("{}", serde_json::to_string_pretty(&info).unwrap());
                return Ok(());
            }
            Ok(Event::Metadata(_, _, Err(e))) => {
                eprintln!("{e}");
                std::process::exit(1)
            }
            _ => {
                stop(&r);
                std::process::exit(3)
            }
        }
    }
    eframe::run_native(
        "Hamster Video",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_icon(
                    eframe::icon_data::from_png_bytes(include_bytes!("../assets/hamster-icon.png"))
                        .expect("bundled icon"),
                )
                .with_inner_size([1120., 870.])
                .with_min_inner_size([960., 820.]),
            ..Default::default()
        },
        Box::new(|cc| Ok(Box::new(Hamster::new(cc)))),
    )
}
#[cfg(test)]
mod tests;
