"""Development application: normal GTK widgets, no test mutation backdoor."""
import gi
gi.require_version('Gtk', '3.0')
from gi.repository import Gtk
window = Gtk.Window(title='Public operations fixture')
window.set_default_size(600, 500)
box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=8)
window.add(box)
status = Gtk.Label(label='Not invoked')
button = Gtk.Button(label='Named operation')
button.connect('clicked', lambda _: status.set_text('Invoked through public action'))
box.pack_start(button, False, False, 0)
box.pack_start(status, False, False, 0)
items = Gtk.ListBox()
items.set_selection_mode(Gtk.SelectionMode.MULTIPLE)
for name in ['Alpha', 'Beta', 'Gamma']:
    row = Gtk.ListBoxRow()
    row.get_accessible().set_name(name)
    row.add(Gtk.Label(label=name))
    items.add(row)
box.pack_start(items, True, True, 0)
scroll = Gtk.Scrollbar(orientation=Gtk.Orientation.HORIZONTAL, adjustment=Gtk.Adjustment(value=20, lower=0, upper=100, step_increment=5, page_increment=10, page_size=0))
scroll.get_accessible().set_name('Public amount')
box.pack_start(scroll, False, False, 0)
editors = {}
for name in ['Text A', 'Text B']:
    frame = Gtk.Frame(label=name)
    frame.get_accessible().set_name(name)
    editor = Gtk.TextView()
    editors[name] = editor
    editor.get_accessible().set_name('Shared text')
    editor.get_buffer().set_text(name + '\noriginal')
    frame.add(editor)
    box.pack_start(frame, True, True, 0)
for name in ['Input A', 'Input B']:
    frame = Gtk.Frame(label=name)
    frame.get_accessible().set_name(name)
    entry = Gtk.Entry()
    entry.get_accessible().set_name('Shared input')
    entry.set_text(name + ' original')
    frame.add(entry)
    box.pack_start(frame, False, False, 0)
dynamic = Gtk.Box()
def toggle_controls(button):
    if dynamic.get_children():
        for child in dynamic.get_children():
            child.destroy()
    else:
        child = Gtk.Button(label='Dynamic operation')
        child.connect('clicked', lambda _: status.set_text('Dynamic invoked'))
        dynamic.add(child)
        dynamic.show_all()
toggle = Gtk.Button(label='Toggle controls')
toggle.connect('clicked', toggle_controls)
box.pack_start(toggle, False, False, 0)
box.pack_start(dynamic, False, False, 0)
def select_range(_):
    buf = editors['Text B'].get_buffer()
    buf.select_range(buf.get_iter_at_offset(0), buf.get_iter_at_offset(1))
select = Gtk.Button(label='Select first character')
select.connect('clicked', select_range)
box.pack_start(select, False, False, 0)
change = Gtk.Button(label='Change amount independently')
change.connect('clicked', lambda _: scroll.get_adjustment().set_value(55))
box.pack_start(change, False, False, 0)
change_text = Gtk.Button(label='Change text independently')
change_text.connect('clicked', lambda _: editors['Text B'].get_buffer().set_text('External text'))
box.pack_start(change_text, False, False, 0)
normalized = Gtk.Scale.new_with_range(Gtk.Orientation.HORIZONTAL, 0, 100, 1)
normalized.get_accessible().set_name('Rounded amount')
def round_value(widget):
    value = widget.get_value()
    rounded = round(value / 5) * 5
    if value != rounded:
        widget.set_value(rounded)
normalized.connect('value-changed', round_value)
box.pack_start(normalized, False, False, 0)
reject_insert = [False]
def reject_new_text(buf, location, text, length):
    if reject_insert[0]:
        buf.stop_emission_by_name('insert-text')
editors['Text B'].get_buffer().connect('insert-text', reject_new_text)
restrict = Gtk.CheckButton(label='Reject inserted text')
restrict.connect('toggled', lambda widget: reject_insert.__setitem__(0, widget.get_active()))
box.pack_start(restrict, False, False, 0)
expander = Gtk.Expander(label='Reveal details')
expander.add(Gtk.Button(label='Revealed operation'))
box.pack_start(expander, False, False, 0)
def open_dialog(_):
    dialog = Gtk.Dialog(title='Scoped confirmation', transient_for=window, modal=True)
    dialog.add_button('Return to task', Gtk.ResponseType.CANCEL)
    dialog.connect('response', lambda dlg, response: dlg.destroy())
    dialog.show_all()
modal = Gtk.Button(label='Open confirmation')
modal.connect('clicked', open_dialog)
box.pack_start(modal, False, False, 0)
window.connect('destroy', Gtk.main_quit)
window.show_all()
Gtk.main()
