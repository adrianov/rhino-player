pub struct SeekPreviewCtx {
    #[cfg(not(target_os = "macos"))]
    pub ovl: gtk::Overlay,
    /// Bottom chrome used for preview lift.
    #[cfg(not(target_os = "macos"))]
    pub bottom: gtk::Box,
}

/// Shared main-player state cells mirrored into the preview state.
pub struct SeekPreviewCells {
    pub player: Rc<RefCell<Option<MpvBundle>>>,
    pub last_path: Rc<RefCell<Option<PathBuf>>>,
    pub enabled: Rc<Cell<bool>>,
    pub chapters: Rc<RefCell<Vec<(f64, String)>>>,
    pub dvd_bar: Rc<RefCell<Option<crate::dvd_vob_timeline::DvdBarState>>>,
}

pub fn connect(
    seek: &gtk::Scale,
    seek_adj: &gtk::Adjustment,
    cells: SeekPreviewCells,
    _ctx: SeekPreviewCtx,
) -> Rc<SeekPreviewState> {
    #[cfg(not(target_os = "macos"))]
    let SeekPreviewCtx { ovl, bottom } = _ctx;
    let SeekPreviewCells {
        player,
        last_path,
        enabled,
        chapters,
        dvd_bar,
    } = cells;
    let (gl, chapter_lbl, time_lbl, container) = build_preview_widgets();
    let (preview, loaded_path, loaded_target, preview_owner_db) = fresh_preview_slots();

    finish_connect(
        seek,
        Rc::new(SeekPreviewState {
            #[cfg(target_os = "macos")]
            popup: macos_popup::build_popup(seek, &container),
            container,
            gl,
            chapter_lbl,
            time_lbl,
            preview,
            loaded_path,
            loaded_target,
            preview_owner_db,
            enabled,
            seek: seek.clone(),
            seek_adj: seek_adj.clone(),
            player,
            last_path,
            chapters,
            dvd_bar,
            hover_t: cell_f64_slot(),
            last_xy: xy_slot(),
            deb: source_slot(),
            shown: Cell::new(false).into(),
            #[cfg(not(target_os = "macos"))]
            bottom,
            #[cfg(not(target_os = "macos"))]
            ovl,
            serial: Cell::new(0).into(),
            pump: source_slot(),
        }),
    )
}

/// Wires GL hooks, macOS popup styling, motion controllers, and global registration.
fn finish_connect(seek: &gtk::Scale, st: Rc<SeekPreviewState>) -> Rc<SeekPreviewState> {
    wire_preview_gl(&st);
    #[cfg(target_os = "macos")]
    macos_popup::wire_opaque_frame(&st);
    wire_motion_controllers(seek, &st);
    register(Rc::clone(&st));
    st
}

/// Fresh `None` source slot (debounce / frame pump).
fn source_slot() -> Rc<RefCell<Option<glib::SourceId>>> {
    Rc::new(RefCell::new(None))
}

fn cell_f64_slot() -> Rc<Cell<f64>> {
    Rc::new(Cell::new(0.0))
}

fn xy_slot() -> Rc<RefCell<Option<(f64, f64)>>> {
    Rc::new(RefCell::new(None))
}

/// GL surface + chapter/time labels inside the floating frame (hidden until first hover).
fn build_preview_widgets() -> (gtk::GLArea, gtk::Label, gtk::Label, gtk::Frame) {
    let gl = preview_gl_area();
    let (chapter_lbl, time_lbl) = preview_labels();
    let container = preview_frame(&gl, &chapter_lbl, &time_lbl);
    (gl, chapter_lbl, time_lbl, container)
}

fn preview_gl_area() -> gtk::GLArea {
    let gl = gtk::GLArea::new();
    gl.set_auto_render(false);
    gl.set_has_stencil_buffer(false);
    gl.set_has_depth_buffer(false);
    gl.set_can_focus(false);
    gl.set_focus_on_click(false);
    gl.set_size_request(180, 101);
    gl
}

/// Chapter name above the numeric hover timestamp.
fn preview_labels() -> (gtk::Label, gtk::Label) {
    let chapter_lbl = gtk::Label::new(None::<&str>);
    chapter_lbl.add_css_class("rp-seek-thumb-chapter");
    chapter_lbl.set_xalign(0.5);
    chapter_lbl.set_ellipsize(gtk::pango::EllipsizeMode::End);
    chapter_lbl.set_max_width_chars(28);
    chapter_lbl.set_visible(false);

    let time_lbl = gtk::Label::new(None::<&str>);
    time_lbl.add_css_class("rp-seek-thumb-time");
    time_lbl.add_css_class("numeric");
    time_lbl.set_xalign(0.5);
    (chapter_lbl, time_lbl)
}

fn preview_frame(gl: &gtk::GLArea, chapter_lbl: &gtk::Label, time_lbl: &gtk::Label) -> gtk::Frame {
    let body = gtk::Box::new(gtk::Orientation::Vertical, 2);
    body.append(gl);
    body.append(chapter_lbl);
    body.append(time_lbl);

    let container = gtk::Frame::new(None::<&str>);
    container.add_css_class("rp-seek-thumb-frame");
    container.set_child(Some(&body));
    container.set_halign(gtk::Align::Start);
    container.set_valign(gtk::Align::End);
    container.set_visible(false);
    container.set_can_target(false);
    container
}

/// Empty caches for the auxiliary player's first load.
type PreviewSlots = (
    Rc<RefCell<Option<MpvPreviewGl>>>,
    Rc<RefCell<Option<PathBuf>>>,
    Rc<RefCell<Option<String>>>,
    Rc<RefCell<Option<PathBuf>>>,
);

fn fresh_preview_slots() -> PreviewSlots {
    (
        Rc::new(RefCell::new(None)),
        path_slot(),
        string_slot(),
        path_slot(),
    )
}

fn path_slot() -> Rc<RefCell<Option<PathBuf>>> {
    Rc::new(RefCell::new(None))
}

fn string_slot() -> Rc<RefCell<Option<String>>> {
    Rc::new(RefCell::new(None))
}

include!("connect_popover_wiring/motion_wiring.rs");
