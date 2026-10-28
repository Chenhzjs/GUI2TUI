"""Controlled public GTK application for document/authority regression only."""
import gi
gi.require_version("Gtk", "3.0")
gi.require_version("Atk", "1.0")
from gi.repository import Gtk, Atk

window = Gtk.Window(title="Shared document validation")
window.set_default_size(900, 900)
box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=4)
window.add(box)
for title in ("Report A", "Report B"):
    frame = Gtk.Frame(label=title)
    frame.get_accessible().set_role(Atk.Role.DOCUMENT_TEXT)
    frame.get_accessible().set_name(title)
    body = Gtk.Box(orientation=Gtk.Orientation.VERTICAL)
    text = Gtk.TextView()
    text.get_accessible().set_name("Shared body")
    text.get_buffer().set_text("\n".join(f"{title} public line {n}" for n in range(240)) + f"\n{title} TAIL")
    text.set_size_request(-1, 80)
    body.pack_start(text, True, True, 0)
    remove = Gtk.Button(label="Remove")
    body.pack_start(remove, False, False, 0)
    frame.add(body)
    box.pack_start(frame, True, True, 0)
password = Gtk.Entry()
password.set_visibility(False)
password.get_accessible().set_name("G2T-P2-SECRET-NAME-8173")
password.set_text("G2T-P2-SECRET-TEXT-8173")
box.pack_start(password, False, False, 0)
readonly = Gtk.TextView()
readonly.set_editable(False)
readonly.get_accessible().set_name("Read only reference")
readonly.get_buffer().set_text("Public read only material")
readonly.set_size_request(-1, 50)
box.pack_start(readonly, True, True, 0)
scale = Gtk.Scale.new_with_range(Gtk.Orientation.HORIZONTAL, 0, 100, 5)
scale.get_accessible().set_name("Public amount")
box.pack_start(scale, False, False, 0)
replace_box = Gtk.Box()
retired_widgets = []
def add_replacement():
    replace_box.pack_start(Gtk.Button(label="Replacement action"), False, False, 0)
    replace_box.show_all()
add_replacement()
def replace_target(_):
    for child in replace_box.get_children():
        # Keep the old public Accessible alive so this regression really creates
        # a different AX object instead of permitting allocator/path reuse.
        retired_widgets.append((child, child.get_accessible()))
        replace_box.remove(child)
        child.destroy()
    add_replacement()
replace = Gtk.Button(label="Replace target identity")
replace.connect("clicked", replace_target)
box.pack_start(replace_box, False, False, 0)
box.pack_start(replace, False, False, 0)
dialogs = []
def open_confirmation(_):
    dialog = Gtk.Dialog(title="Scoped confirmation", transient_for=window, modal=True)
    dialog.add_button("Cancel confirmation", Gtk.ResponseType.CANCEL)
    dialogs.append(dialog)
    def close(d, _):
        dialogs.remove(d)
        d.destroy()
    dialog.connect("response", close)
    dialog.show_all()
modal = Gtk.Button(label="Open confirmation")
modal.connect("clicked", open_confirmation)
box.pack_start(modal, False, False, 0)
window.connect("destroy", Gtk.main_quit)
window.show_all()
Gtk.main()
