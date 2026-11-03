//! X11 file clipboard: a selection owner that answers paste requests.
//!
//! The X11 clipboard is not storage. `CLIPBOARD` has an owner window, and a
//! paste is a round trip: the pasting client calls `ConvertSelection`, the
//! server forwards a `SelectionRequest` to the owner, the owner writes the data
//! into a property of the requester and sends `SelectionNotify` back. So a copy
//! needs a client that keeps running after the copy call returns:
//!
//! - [`set`] connects, owns an unmapped `InputOnly` window, takes the
//!   `CLIPBOARD` selection and then hands the connection to a dedicated thread;
//! - the thread answers every advertised target from the payload built once per
//!   copy ([`Payload`]), so an image is encoded once rather than per request;
//! - payloads larger than one request are streamed in `INCR` chunks;
//! - `SelectionClear` — another application copied something — ends the thread.
//!
//! The bytes exist only while this process owns the selection, so quitting fist
//! empties the clipboard unless a clipboard manager (clipmenud, clipman, CopyQ,
//! klipper, …) has taken the selection over.

use std::thread;

use x11rb::connection::{Connection, RequestConnection};
use x11rb::errors::{ConnectionError, ReplyError, ReplyOrIdError};
use x11rb::protocol::Event;
use x11rb::protocol::xproto::{
    Atom, AtomEnum, ChangeWindowAttributesAux, ConnectionExt, CreateWindowAux, EventMask, PropMode,
    Property, PropertyNotifyEvent, SELECTION_NOTIFY_EVENT, SelectionNotifyEvent,
    SelectionRequestEvent, Window, WindowClass,
};
use x11rb::rust_connection::RustConnection;

use super::{Clip, CopyMode, Error};

impl From<ConnectionError> for Error {
    fn from(e: ConnectionError) -> Self {
        Error(e.to_string())
    }
}

impl From<ReplyError> for Error {
    fn from(e: ReplyError) -> Self {
        Error(e.to_string())
    }
}

impl From<ReplyOrIdError> for Error {
    fn from(e: ReplyOrIdError) -> Self {
        Error(e.to_string())
    }
}

/// Own the `CLIPBOARD` selection and serve `clip` from a dedicated thread until
/// another client takes the selection over.
pub fn set(clip: Clip) -> Result<(), Error> {
    let (conn, screen_num) = x11rb::connect(None).map_err(|e| Error(e.to_string()))?;
    let screen = &conn.setup().roots[screen_num];
    let atoms = Atoms::intern(&conn)?;

    // An InputOnly window can carry properties but never appears on screen.
    let window = conn.generate_id()?;
    conn.create_window(
        0,
        window,
        screen.root,
        0,
        0,
        1,
        1,
        0,
        WindowClass::INPUT_ONLY,
        0,
        &CreateWindowAux::new().event_mask(EventMask::PROPERTY_CHANGE),
    )?;
    let time = server_time(&conn, window, AtomEnum::STRING.into())?;
    conn.set_selection_owner(window, atoms.clipboard, time)?
        .check()?;
    if conn.get_selection_owner(atoms.clipboard)?.reply()?.owner != window {
        return Err(Error("CLIPBOARD was taken by another client".into()));
    }

    let payload = Payload::new(clip, time);
    thread::Builder::new()
        .name("x11-clipboard".into())
        .spawn(move || {
            if let Err(e) = serve(&conn, &atoms, &payload) {
                log::warn!("X11 clipboard server stopped: {e}");
            }
        })
        .map_err(|e| Error(format!("spawn clipboard thread: {e}")))?;
    Ok(())
}

/// A real server timestamp. `SetSelectionOwner` is ignored when the time is
/// older than the current owner's, so the owner asks the server for one by
/// appending an empty property to its own window and reading the event time.
fn server_time(
    conn: &RustConnection,
    window: Window,
    property: Atom,
) -> Result<u32, Error> {
    conn.change_property(
        PropMode::APPEND,
        window,
        property,
        AtomEnum::STRING,
        8,
        0,
        &[],
    )?;
    // waiting does not send buffered requests: the timestamp request itself has
    // to reach the server before its event can arrive
    conn.flush()?;
    loop {
        if let Event::PropertyNotify(event) = conn.wait_for_event()? {
            if event.atom == property {
                return Ok(event.time);
            }
        }
    }
}

/// The atoms one copy needs; the predefined ones come from [`AtomEnum`].
struct Atoms {
    clipboard: Atom,
    targets: Atom,
    timestamp: Atom,
    uri_list: Atom,
    gnome_copied: Atom,
    kde_cut: Atom,
    utf8_string: Atom,
    text: Atom,
    text_utf8: Atom,
    png: Atom,
    incr: Atom,
}

impl Atoms {
    fn intern(conn: &RustConnection) -> Result<Self, Error> {
        let intern = |name: &[u8]| -> Result<Atom, Error> {
            Ok(conn.intern_atom(false, name)?.reply()?.atom)
        };
        Ok(Self {
            clipboard: intern(b"CLIPBOARD")?,
            targets: intern(b"TARGETS")?,
            timestamp: intern(b"TIMESTAMP")?,
            uri_list: intern(b"text/uri-list")?,
            gnome_copied: intern(b"x-special/gnome-copied-files")?,
            kde_cut: intern(b"application/x-kde-cutselection")?,
            utf8_string: intern(b"UTF8_STRING")?,
            text: intern(b"text/plain")?,
            text_utf8: intern(b"text/plain;charset=utf-8")?,
            png: intern(b"image/png")?,
            incr: intern(b"INCR")?,
        })
    }
}

/// Everything the serving thread hands out for one copy.
struct Payload {
    timestamp: u32,
    uri_list: String,
    gnome_copied: String,
    text: String,
    png: Option<Vec<u8>>,
    cut: bool,
}

impl Payload {
    fn new(
        clip: Clip,
        timestamp: u32,
    ) -> Self {
        Self {
            timestamp,
            uri_list: super::uri_list(&clip.paths),
            gnome_copied: super::gnome_copied(&clip.paths, clip.mode),
            text: clip.text,
            png: clip.png,
            cut: clip.mode == CopyMode::Cut,
        }
    }

    /// The advertised targets: file references first, then the flavours a
    /// terminal or an image editor asks for.
    fn targets(
        &self,
        atoms: &Atoms,
    ) -> Vec<Atom> {
        let mut targets = vec![
            atoms.targets,
            atoms.timestamp,
            atoms.uri_list,
            atoms.gnome_copied,
            atoms.kde_cut,
            atoms.utf8_string,
            atoms.text_utf8,
            atoms.text,
        ];
        if self.text.is_ascii() {
            // STRING is latin-1, so it is only advertised when that is true
            targets.push(AtomEnum::STRING.into());
        }
        if self.png.is_some() {
            targets.push(atoms.png);
        }
        targets
    }

    /// `(format, bytes)` for `target`, or `None` when it is not advertised.
    fn data(
        &self,
        atoms: &Atoms,
        target: Atom,
    ) -> Option<(u8, Vec<u8>)> {
        let string: Atom = AtomEnum::STRING.into();
        if target == atoms.targets {
            let targets = self.targets(atoms);
            return Some((32, targets.iter().flat_map(|a| a.to_ne_bytes()).collect()));
        }
        if target == atoms.timestamp {
            return Some((32, self.timestamp.to_ne_bytes().to_vec()));
        }
        if target == atoms.uri_list {
            return Some((8, self.uri_list.as_bytes().to_vec()));
        }
        if target == atoms.gnome_copied {
            return Some((8, self.gnome_copied.as_bytes().to_vec()));
        }
        if target == atoms.kde_cut {
            let marker: &[u8] = if self.cut { b"1" } else { b"0" };
            return Some((8, marker.to_vec()));
        }
        if target == atoms.png {
            return self.png.as_ref().map(|png| (8, png.clone()));
        }
        if target == atoms.utf8_string || target == atoms.text || target == atoms.text_utf8 {
            return Some((8, self.text.as_bytes().to_vec()));
        }
        if target == string && self.text.is_ascii() {
            return Some((8, self.text.as_bytes().to_vec()));
        }
        None
    }
}

/// Answer paste requests until the selection is taken over.
fn serve(
    conn: &RustConnection,
    atoms: &Atoms,
    payload: &Payload,
) -> Result<(), Error> {
    // Leave room for the request header, so one property write stays legal.
    let max_bytes = conn.maximum_request_bytes().saturating_sub(64);
    let mut transfers: Vec<Incr> = Vec::new();
    let mut cleared = false;
    loop {
        match conn.wait_for_event()? {
            Event::SelectionRequest(request) => {
                if !cleared {
                    handle_request(conn, atoms, payload, &request, max_bytes, &mut transfers)?;
                } else {
                    notify(conn, &request, 0)?;
                }
            }
            Event::SelectionClear(_) => {
                if transfers.is_empty() {
                    return Ok(());
                }
                cleared = true;
            }
            Event::PropertyNotify(event) => {
                let mut i = 0;
                while i < transfers.len() {
                    match transfers[i].advance(conn, &event, max_bytes) {
                        Ok(true) => {
                            let _ = conn.change_window_attributes(
                                transfers[i].requestor,
                                &ChangeWindowAttributesAux::new().event_mask(EventMask::NO_EVENT),
                            );
                            transfers.remove(i);
                        }
                        Ok(false) => i += 1,
                        Err(e) => {
                            log::warn!("X11 clipboard transfer aborted: {e}");
                            let _ = conn.change_window_attributes(
                                transfers[i].requestor,
                                &ChangeWindowAttributesAux::new().event_mask(EventMask::NO_EVENT),
                            );
                            transfers.remove(i);
                        }
                    }
                }
                if cleared && transfers.is_empty() {
                    return Ok(());
                }
            }
            _ => {}
        }
    }
}

/// Serve one `SelectionRequest`: write the data into the requester's property
/// and notify it, or reply with `property = None` for an unsupported target.
fn handle_request(
    conn: &RustConnection,
    atoms: &Atoms,
    payload: &Payload,
    request: &SelectionRequestEvent,
    max_bytes: usize,
    transfers: &mut Vec<Incr>,
) -> Result<(), Error> {
    // obsolete clients send a `None` property and expect the target name
    let property = if request.property == 0 {
        request.target
    } else {
        request.property
    };
    let Some((format, bytes)) = payload.data(atoms, request.target) else {
        return notify(conn, request, 0);
    };
    let type_ = if request.target == atoms.targets {
        AtomEnum::ATOM.into()
    } else if request.target == atoms.timestamp {
        AtomEnum::INTEGER.into()
    } else {
        request.target
    };
    if bytes.len() <= max_bytes {
        conn.change_property(
            PropMode::REPLACE,
            request.requestor,
            property,
            type_,
            format,
            units(format, bytes.len()),
            &bytes,
        )?;
        return notify(conn, request, property);
    }
    // A payload that does not fit one request is streamed: the marker carries
    // the total size, then each chunk is written when the requester deletes the
    // previous one. The property-change mask has to be in place first, or the
    // requester deletes the marker before we would hear about it.
    conn.change_window_attributes(
        request.requestor,
        &ChangeWindowAttributesAux::new().event_mask(EventMask::PROPERTY_CHANGE),
    )?;
    let total = (bytes.len() as u32).to_ne_bytes();
    conn.change_property(
        PropMode::REPLACE,
        request.requestor,
        property,
        atoms.incr,
        32,
        1,
        &total,
    )?;
    notify(conn, request, property)?;
    transfers.push(Incr {
        requestor: request.requestor,
        property,
        type_,
        format,
        data: bytes,
        offset: 0,
        terminated: false,
    });
    Ok(())
}

/// A property write counts `data_len` in units of `format`, not bytes.
fn units(
    format: u8,
    bytes: usize,
) -> u32 {
    (bytes / usize::from(format / 8)) as u32
}

/// Send the `SelectionNotify` that closes a request; `property = None` reports
/// an unsupported target.
fn notify(
    conn: &RustConnection,
    request: &SelectionRequestEvent,
    property: Atom,
) -> Result<(), Error> {
    let event = SelectionNotifyEvent {
        response_type: SELECTION_NOTIFY_EVENT,
        time: request.time,
        requestor: request.requestor,
        selection: request.selection,
        target: request.target,
        property,
        ..Default::default()
    };
    conn.send_event(false, request.requestor, EventMask::NO_EVENT, event)?;
    conn.flush()?;
    Ok(())
}

/// An `INCR` transfer in flight. The requester deletes each chunk after reading
/// it, which is what triggers the next write; a zero-length property ends the
/// stream.
struct Incr {
    requestor: Window,
    property: Atom,
    type_: Atom,
    format: u8,
    data: Vec<u8>,
    offset: usize,
    terminated: bool,
}

impl Incr {
    /// Write the next chunk. Returns whether the transfer is over.
    fn advance(
        &mut self,
        conn: &RustConnection,
        event: &PropertyNotifyEvent,
        max_bytes: usize,
    ) -> Result<bool, Error> {
        if event.window != self.requestor
            || event.atom != self.property
            || event.state != Property::DELETE
        {
            return Ok(false);
        }
        if self.terminated {
            return Ok(true);
        }
        if self.offset < self.data.len() {
            let end = (self.offset + max_bytes).min(self.data.len());
            let chunk = &self.data[self.offset..end];
            conn.change_property(
                PropMode::REPLACE,
                self.requestor,
                self.property,
                self.type_,
                self.format,
                units(self.format, chunk.len()),
                chunk,
            )?;
            self.offset = end;
        } else {
            conn.change_property(
                PropMode::REPLACE,
                self.requestor,
                self.property,
                self.type_,
                self.format,
                0,
                &[],
            )?;
            self.terminated = true;
        }
        conn.flush()?;
        Ok(false)
    }
}
