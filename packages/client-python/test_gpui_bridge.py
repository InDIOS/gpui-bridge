# Runs against the real host binary in headless mode (build it first: `cargo build` in host/).
# Run: python -m unittest   (from client-python/)

import os
import sys
import unittest
from pathlib import Path

import gpui_bridge
from gpui_bridge import App, HostError, HostExitedError


class ClientTest(unittest.TestCase):
    def test_handshake_completes(self):
        app = App(headless=True)
        self.assertEqual(app.close().code, 0)

    def test_valid_tree_commits_without_errors(self):
        app = App(headless=True)
        errors = []
        app.on_error = errors.append

        root = app.div(class_="flex flex-col gap-2 p-4")
        label = app.text(content="Clicks: 0")
        button = app.button(label="Add", variant="primary").on("click", lambda: None)
        input = app.input(placeholder="Type").on("change", print).on("submit", print)
        checkbox = app.checkbox(label="Done", controlled=True).on("change", print)
        root.append(label, button, input, checkbox)
        app.window(title="Test", width=400, height=300, root=root, theme="dark")
        app.commit()

        label.set(content="Clicks: 1")
        button.set(variant=None, disabled=True).off("click")
        checkbox.remove()
        app.commit()

        # The host handles lines in order, so every reply precedes its exit.
        app.close()
        self.assertEqual(errors, [])

    def test_an_image_can_carry_base64_bytes(self):
        import base64

        app = App(headless=True)
        errors = []
        app.on_error = errors.append

        image = app.image(data=base64.b64encode(b"hello").decode("ascii"), fit="cover")
        app.window(root=app.div().append(image))
        app.commit()

        # Bytes sent again reload the node; a bad payload is rejected and the previous bytes stay.
        image.set(data=base64.b64encode(b"world").decode("ascii"))
        app.commit()
        image.set(data="not base64!")
        app.commit()

        app.close()
        self.assertEqual(
            [(error.code, error.message) for error in errors],
            [("invalid_props", "Prop 'data' of Image must be standard base64")],
        )

    def test_host_errors_arrive_typed_and_host_keeps_running(self):
        app = App(headless=True)
        errors = []
        app.on_error = errors.append

        root = app.div()
        gone = app.text(content="gone")
        gone.remove()
        root.append(gone)  # the host no longer knows this node
        app.commit()

        app.button(label="x").set(disabled="yes")
        app.commit()

        app.close()
        self.assertEqual(
            errors,
            [
                HostError("unknown_node", f"Node {gone.id} does not exist", "append"),
                HostError("invalid_props", "Prop 'disabled' of Button must be a boolean, got \"yes\"", "setProps"),
            ],
        )

    def test_events_are_dispatched_with_payloads(self):
        app = App(headless=True)
        seen = []
        button = app.button().on("click", lambda: seen.append("click"))
        input = app.input().on("change", lambda value: seen.append(("change", value)))
        checkbox = app.checkbox().on("change", lambda checked: seen.append(("checked", checked)))

        # Simulate host events (real clicks need a window; see the end-to-end check).
        app._dispatch({"type": "event", "id": button.id, "event": "click"})
        app._dispatch({"type": "event", "id": input.id, "event": "change", "value": "hi"})
        app._dispatch({"type": "event", "id": checkbox.id, "event": "change", "checked": True})
        input.off("change")
        app._dispatch({"type": "event", "id": input.id, "event": "change", "value": "ignored"})

        app.close()
        self.assertEqual(seen, ["click", ("change", "hi"), ("checked", True)])

    def test_menus_are_accepted_and_report_the_chosen_item(self):
        app = App(headless=True)
        errors = []
        app.on_error = errors.append
        seen = []
        bar = app.menu_bar(menus=[{"label": "File", "items": [{"id": "open", "label": "Open"}, {"separator": True}]}])
        bar.on("menu", lambda item: seen.append(("bar", item)))
        area = app.div(contextMenu=[{"id": "copy", "label": "Copy"}]).on("menu", lambda item: seen.append(("area", item)))
        app.window(root=app.div().append(bar, area))
        app.commit()

        app._dispatch({"type": "event", "id": bar.id, "event": "menu", "value": "open"})
        app._dispatch({"type": "event", "id": area.id, "event": "menu", "value": "copy"})
        app.close()
        self.assertEqual(errors, [])
        self.assertEqual(seen, [("bar", "open"), ("area", "copy")])

    def test_tray_reports_events_and_removing_it_with_no_window_ends_the_host(self):
        app = App(headless=True)
        errors, seen = [], []
        app.on_error = errors.append
        app.on_tray = lambda event, item: seen.append((event, item))
        app.tray(tooltip="Test", menu=[{"id": "quit", "label": "Quit"}]).commit()

        app._dispatch({"type": "tray", "event": "click"})
        app._dispatch({"type": "tray", "event": "menu", "value": "quit"})
        app.remove_tray().commit()
        exit = app.run()
        self.assertEqual(exit.code, 0)
        self.assertEqual(errors, [])
        self.assertEqual(seen, [("click", None), ("menu", "quit")])

    def test_form_controls_are_accepted_and_report_typed_values(self):
        app = App(headless=True)
        errors, seen = [], []
        app.on_error = errors.append
        options = [{"value": "a", "label": "A"}, {"value": "b", "label": "B", "disabled": True}]
        toggle = app.switch(label="On", checked=True).on("change", lambda checked: seen.append(("switch", checked)))
        radios = app.radio_group(options=options, value="a").on("change", lambda value: seen.append(("radio", value)))
        select = app.select(options=options, searchable=True).on("change", lambda value: seen.append(("select", value)))
        slider = app.slider(min=0, max=1, step=0.1, value=0.5).on("change", lambda value: seen.append(("slider", value)))
        notes = app.textarea(placeholder="Notes").on("change", lambda value: seen.append(("notes", value)))
        app.window(root=app.div().append(toggle, radios, select, slider, notes))
        app.commit()

        app._dispatch({"type": "event", "id": toggle.id, "event": "change", "checked": False})
        app._dispatch({"type": "event", "id": radios.id, "event": "change", "value": "a"})
        app._dispatch({"type": "event", "id": select.id, "event": "change", "value": "a"})
        app._dispatch({"type": "event", "id": slider.id, "event": "change", "value": 0.3})
        app._dispatch({"type": "event", "id": notes.id, "event": "change", "value": "1\n2"})
        app.close()
        self.assertEqual(errors, [])
        self.assertEqual(seen, [("switch", False), ("radio", "a"), ("select", "a"), ("slider", 0.3), ("notes", "1\n2")])
    def test_new_components_are_accepted_and_report_their_events(self):
        app = App(headless=True)
        errors, seen = [], []
        app.on_error = errors.append
        rating = app.rating(value=2).on("change", lambda value: seen.append(("rating", value)))
        pages = app.pagination(page=1, total=4).on("change", lambda value: seen.append(("page", value)))
        accordion = app.accordion(
            items=[{"id": "a", "title": "First", "content": "Body", "open": True}, {"id": "b", "title": "Second"}]
        ).on("change", lambda value: seen.append(("accordion", value)))
        steps = app.stepper(
            value="one", items=[{"value": "one", "label": "One"}, {"value": "two", "label": "Two"}]
        ).on("change", lambda value: seen.append(("step", value)))
        otp = app.otp_input(length=6, masked=True).on("change", lambda value: seen.append(("otp", value)))
        day = app.calendar(value="2024-08-03").on("change", lambda value: seen.append(("day", value)))
        field = app.field(label="Name", required=True).append(app.input(placeholder="Ana"))
        form = app.form(direction="vertical").append(field)
        details = app.group_box(title="Details").append(
            app.description_list(items=[{"label": "Plan", "value": "Pro"}])
        )
        panels = app.resizable(axis="horizontal", sizes=[200]).append(
            app.div(a11y={"role": "region", "label": "Side"}),
            app.div(a11y={"role": "region", "label": "Content"}),
        )
        root = app.div().append(
            app.alert(message="Saved", variant="success", closable=True),
            app.badge(count=3),
            app.tag(label="beta", variant="info"),
            app.kbd(keys="ctrl-s"),
            app.progress(value=40),
            app.spinner(),
            app.skeleton(),
            rating,
            pages,
            app.collapsible(open=True).append(app.text(content="Hidden")),
            accordion,
            app.avatar(name="Ana"),
            steps,
            app.number_input(value="12"),
            otp,
            day,
            form,
            details,
            app.toolbar().append(app.button(label="Bold")),
            panels,
            app.status_bar(left="Ready", right="UTF-8"),
        )
        app.window(root=root)
        app.commit()

        app._dispatch({"type": "event", "id": rating.id, "event": "change", "value": 4})
        app._dispatch({"type": "event", "id": pages.id, "event": "change", "value": 2})
        app._dispatch({"type": "event", "id": accordion.id, "event": "change", "value": "b"})
        app._dispatch({"type": "event", "id": steps.id, "event": "change", "value": "two"})
        app._dispatch({"type": "event", "id": otp.id, "event": "change", "value": "123456"})
        app._dispatch({"type": "event", "id": day.id, "event": "change", "value": "2024-09-01"})
        app.close()
        self.assertEqual(errors, [])
        self.assertEqual(
            seen,
            [
                ("rating", 4),
                ("page", 2),
                ("accordion", "b"),
                ("step", "two"),
                ("otp", "123456"),
                ("day", "2024-09-01"),
            ],
        )

    def test_simple_components_are_accepted_and_report_their_events(self):
        app = App(headless=True)
        errors, seen = [], []
        app.on_error = errors.append
        toggle = app.toggle(label="Bold", icon="asterisk", variant="outline", tooltip="Bold").on(
            "change", lambda checked: seen.append(("toggle", checked))
        )
        save = (
            app.dropdown_button(
                label="Save",
                variant="primary",
                items=[{"id": "save-as", "label": "Save as"}],
                menuLabel="Save options",
            )
            .on("click", lambda: seen.append(("save",)))
            .on("menu", lambda item: seen.append(("menu", item)))
        )
        group = (
            app.input_group(value="example.com", placeholder="host", prefix="https://", suffixIcon="check")
            .on("change", lambda value: seen.append(("group", value)))
            .on("submit", lambda value: seen.append(("submit", value)))
        )
        clipboard = app.clipboard(value="https://example.com", a11y={"label": "Copy the address"}).on(
            "copied", lambda value: seen.append(("copied", value))
        )
        root = app.div(class_="flex flex-col gap-2 p-4").append(
            toggle,
            save,
            app.label(content="Total", secondary="(12)", highlight="12"),
            app.empty(icon="folder-open", title="No files", description="Drop a file here").append(
                app.button(label="Choose a file")
            ),
            app.shimmer(content="Loading...", duration=3, spread=0.4, reverse=True, once=True),
            group,
            clipboard,
        )
        app.window(root=root)
        app.commit()

        app._dispatch({"type": "event", "id": toggle.id, "event": "change", "checked": True})
        app._dispatch({"type": "event", "id": save.id, "event": "click"})
        app._dispatch({"type": "event", "id": save.id, "event": "menu", "value": "save-as"})
        app._dispatch({"type": "event", "id": group.id, "event": "change", "value": "example.org"})
        app._dispatch({"type": "event", "id": group.id, "event": "submit", "value": "example.org"})
        app._dispatch({"type": "event", "id": clipboard.id, "event": "copied", "value": "https://example.com"})
        app.close()
        self.assertEqual(errors, [])
        self.assertEqual(
            seen,
            [
                ("toggle", True),
                ("save",),
                ("menu", "save-as"),
                ("group", "example.org"),
                ("submit", "example.org"),
                ("copied", "https://example.com"),
            ],
        )

    def test_overlays_are_accepted_and_report_their_events(self):
        app = App(headless=True)
        errors, seen = [], []
        app.on_error = errors.append
        alert = (
            app.alert_dialog(
                open=True,
                title="Delete 3 files?",
                description="This cannot be undone.",
                icon="circle-alert",
                okText="Delete",
                cancelText="Cancel",
            )
            .on("ok", lambda: seen.append(("ok",)))
            .append(app.text(content="3 files"))
        )
        sheet = (
            app.sheet(open=True, side="left", title="Filters", size=320)
            .on("close", lambda: seen.append(("close",)))
            .append(app.input(value="*.png"))
        )
        popover = (
            app.popover(open=False, anchor="bottom-right", arrow=True)
            .on("change", lambda checked: seen.append(("popover", checked)))
            .append(app.button(label="Sort"), app.text(content="By name"))
        )
        card = (
            app.hover_card(openDelay=300, closeDelay=200, anchor="top-center")
            .on("change", lambda checked: seen.append(("card", checked)))
            .append(app.button(label="Info"), app.text(content="Hovered"))
        )
        app.window(root=app.div(class_="flex flex-col gap-2 p-4").append(alert, sheet, popover, card))
        app.commit()

        app._dispatch({"type": "event", "id": alert.id, "event": "ok"})
        app._dispatch({"type": "event", "id": sheet.id, "event": "close"})
        app._dispatch({"type": "event", "id": popover.id, "event": "change", "checked": True})
        app._dispatch({"type": "event", "id": card.id, "event": "change", "checked": False})
        app.close()
        self.assertEqual(errors, [])
        self.assertEqual(seen, [("ok",), ("close",), ("popover", True), ("card", False)])

    def test_chat_nodes_are_accepted_and_report_their_events(self):
        app = App(headless=True)
        errors = []
        app.on_error = errors.append
        seen = []

        file = app.attachment(status="failed", title="report.pdf", description="2.4 MB")
        file.on("click", lambda: seen.append("click")).on("remove", lambda: seen.append("remove"))
        file.on("retry", lambda: seen.append("retry"))
        quiz = app.questionnaire(
            items=[
                {"name": "plan", "label": "Plan", "choices": [{"value": "free", "label": "Free"}]},
                {"name": "notes", "label": "Notes", "input": True},
            ]
        )
        quiz.on("change", lambda value: seen.append(("change", value)))
        quiz.on("submit", lambda value: seen.append(("submit", value)))
        message = app.message(alignment="end", avatarName="Ada").append(
            app.message_header().append(app.text(content="Ada")),
            app.bubble(variant="secondary").append(app.text(content="Hi")),
            app.message_footer(),
        )
        scroller = app.message_scroller(jumpLabel="Latest").append(
            message, app.marker(content="Thinking", loading=True, loadingStyle="shimmer"), file, quiz
        )
        app.window(root=app.div(class_="size-full flex flex-col").append(scroller))
        app.commit()

        answer = {"name": "plan", "status": "answered", "choices": ["free"], "text": None}
        app._dispatch({"type": "event", "id": file.id, "event": "click"})
        app._dispatch({"type": "event", "id": file.id, "event": "remove"})
        app._dispatch({"type": "event", "id": file.id, "event": "retry"})
        app._dispatch({"type": "event", "id": quiz.id, "event": "change", "value": answer})
        app._dispatch({"type": "event", "id": quiz.id, "event": "submit", "value": [answer]})

        app.close()
        self.assertEqual(errors, [])
        self.assertEqual(seen, ["click", "remove", "retry", ("change", answer), ("submit", [answer])])

    def test_list_is_accepted_and_reports_its_events(self):
        app = App(headless=True)
        errors = []
        app.on_error = errors.append
        seen = []

        rows = app.list(value="a", searchable=True, placeholder="Search", hasMore=True, loadMoreThreshold=5)
        rows.on("change", lambda value: seen.append(("change", value)))
        rows.on("confirm", lambda value: seen.append(("confirm", value)))
        rows.on("query", lambda value: seen.append(("query", value)))
        rows.on("loadMore", lambda: seen.append("loadMore"))
        rows.append(
            app.list_item(value="a").append(app.text(content="Apple")),
            app.list_item(value="b", disabled=True).append(app.text(content="Banana")),
        )
        app.window(root=app.div(class_="size-full flex flex-col").append(rows))
        app.commit()

        app._dispatch({"type": "event", "id": rows.id, "event": "change", "value": None})
        app._dispatch({"type": "event", "id": rows.id, "event": "confirm", "value": "a"})
        app._dispatch({"type": "event", "id": rows.id, "event": "query", "value": "ap"})
        app._dispatch({"type": "event", "id": rows.id, "event": "loadMore"})

        app.close()
        self.assertEqual(errors, [])
        self.assertEqual(seen, [("change", None), ("confirm", "a"), ("query", "ap"), "loadMore"])

    def test_dock_is_accepted_and_reports_close(self):
        app = App(headless=True)
        errors = []
        app.on_error = errors.append
        seen = []

        notes = app.dock_panel(title="Notes", closable=True)
        notes.on("close", lambda: seen.append("close"))
        dock = app.dock(class_="size-full").append(
            app.dock_panel(title="Files", placement="left").append(app.text(content="src/")),
            notes.append(app.text(content="Hello")),
        )
        app.window(root=dock)
        app.commit()

        app._dispatch({"type": "event", "id": notes.id, "event": "close"})

        app.close()
        self.assertEqual(errors, [])
        self.assertEqual(seen, ["close"])

    def test_form_controls_with_state_are_accepted_and_report_typed_values(self):
        app = App(headless=True)
        errors = []
        app.on_error = errors.append
        seen = []

        options = [
            {"value": "rs", "label": "Rust"},
            {"value": "go", "label": "Go"},
            {"value": "py", "label": "Python", "disabled": True},
        ]
        day = app.date_picker(value="2024-08-03", placeholder="Pick a day").on("change", lambda value: seen.append(("day", value)))
        range_ = app.date_picker(range=True).on("change", lambda value: seen.append(("range", value)))
        time_ = app.time_field(value="09:30").on("change", lambda value: seen.append(("time", value)))
        color = app.color_picker(value="#3366ff", palette=["#ff0000", "#00ff0080"]).on(
            "change", lambda value: seen.append(("color", value))
        )
        langs = app.combobox(options=options, value=["rs"], placeholder="Languages", searchable=True).on(
            "change", lambda value: seen.append(("langs", value))
        )
        app.window(root=app.div().append(day, range_, time_, color, langs))
        app.commit()

        app._dispatch({"type": "event", "id": day.id, "event": "change", "value": "2024-08-10"})
        app._dispatch({"type": "event", "id": day.id, "event": "change", "value": None})
        app._dispatch({"type": "event", "id": range_.id, "event": "change", "value": "2024-08-01/2024-08-15"})
        app._dispatch({"type": "event", "id": time_.id, "event": "change", "value": "18:05"})
        app._dispatch({"type": "event", "id": color.id, "event": "change", "value": "#FF0000"})
        app._dispatch({"type": "event", "id": color.id, "event": "change", "value": None})
        app._dispatch({"type": "event", "id": langs.id, "event": "change", "value": ["rs", "go"]})
        app._dispatch({"type": "event", "id": langs.id, "event": "change", "value": ["go"]})

        app.close()
        self.assertEqual(errors, [])
        self.assertEqual(
            seen,
            [
                ("day", "2024-08-10"),
                ("day", None),
                ("range", "2024-08-01/2024-08-15"),
                ("time", "18:05"),
                ("color", "#FF0000"),
                ("color", None),
                ("langs", ["rs", "go"]),
                ("langs", ["go"]),
            ],
        )

    def test_an_a11y_prop_is_validated_by_the_host(self):
        app = App(headless=True)
        errors = []
        app.on_error = errors.append
        ok = app.div(a11y={"id": "save", "role": "button", "label": "Save", "keyshortcuts": "ctrl-s"})
        app.div(a11y={"lable": "typo"})  # rejected; it is not appended, so the host never has it
        app.window(root=app.div().append(ok))
        app.commit()
        ok.set(a11y=None)
        app.commit()
        app.close()
        self.assertEqual([err.code for err in errors], ["invalid_props"])



    def test_shortcuts_keydown_doubleclick_and_focus(self):
        app = App(headless=True)
        errors, seen = [], []
        app.on_error = errors.append
        app.on_shortcut = lambda keys, window: seen.append(("shortcut", keys, window))
        area = app.div().on("keyDown", lambda key: seen.append(("key", key))).on("doubleClick", lambda: seen.append(("double",)))
        search = app.input()
        app.window(root=app.div().append(area, search))
        app.shortcuts(["f5"])
        search.focus()
        app.commit()

        app._dispatch({"type": "shortcut", "keys": "f5", "window": 1})
        app._dispatch({"type": "event", "id": area.id, "event": "keyDown", "value": "escape"})
        app._dispatch({"type": "event", "id": area.id, "event": "doubleClick"})
        app.close()
        self.assertEqual(errors, [])
        self.assertEqual(seen, [("shortcut", "f5", 1), ("key", "escape"), ("double",)])

    def test_system_actions_get_their_results_and_drop_delivers_paths(self):
        app = App(headless=True)
        errors, dropped, clicks = [], [], []
        app.on_error = errors.append
        area = app.div().on("drop", dropped.append)
        button = app.button().on("click", lambda: clicks.append(1))
        app.window(root=app.div().append(area, button))
        app.commit()

        # Headless, dialogs and the clipboard answer None.
        self.assertIsNone(app.open_dialog(multiple=True, prompt="Open"))
        self.assertIsNone(app.save_dialog(name="new.txt"))
        app.write_clipboard("hi")
        app.open_url("https://example.com")
        self.assertIsNone(app.read_clipboard())
        with self.assertRaisesRegex(RuntimeError, "host rejected 'openDialog'"):
            app.open_dialog(files=False)
        app._dispatch({"type": "event", "id": area.id, "event": "drop", "paths": ["C:\\a.txt"]})
        app.close()
        self.assertEqual(errors, [])
        self.assertEqual(dropped, [["C:\\a.txt"]])

    def test_dialog_notify_and_tooltip(self):
        app = App(headless=True)
        errors, seen = [], []
        app.on_error = errors.append
        app.on_notification = lambda id, window: seen.append(("notification", id, window))
        dialog = app.dialog(open=True, title="Rename", okText="Save", cancelText="Cancel").append(app.input(value="x"))
        dialog.on("ok", lambda: seen.append(("ok",))).on("cancel", lambda: seen.append(("cancel",)))
        app.window(root=app.div(tooltip="Area").append(dialog, app.button(label="Save", tooltip="Ctrl+S")))
        app.commit()
        app.notify("Saved", title="Done", type="success", id="saved")

        app._dispatch({"type": "event", "id": dialog.id, "event": "ok"})
        app._dispatch({"type": "event", "id": dialog.id, "event": "cancel"})
        app._dispatch({"type": "notification", "id": "saved", "window": 1})
        app.close()
        self.assertEqual(errors, [])
        self.assertEqual(seen, [("ok",), ("cancel",), ("notification", "saved", 1)])

    def test_virtual_list_table_and_tabs(self):
        app = App(headless=True)
        errors, seen = [], []
        app.on_error = errors.append
        rows = app.virtual_list(rowHeight=24, class_="h-64")
        for i in range(500):
            rows.append(app.text(content=f"Row {i}"))
        table = app.table(columns=[{"key": "name", "name": "Name", "sortable": True}], rows=[{"name": "a"}, {"name": "b"}], selected=0)
        table.on("change", lambda row: seen.append(("row", row))).on("doubleClick", lambda row: seen.append(("open", row)))
        table.on("sort", lambda sort: seen.append(("sort", sort)))
        tabs = app.tabs(tabs=[{"value": "list", "label": "List"}, {"value": "table", "label": "Table"}], value="list", variant="pill")
        tabs.on("change", lambda value: seen.append(("tab", value)))
        app.window(root=app.div().append(tabs, rows, table))
        app.commit()

        app._dispatch({"type": "event", "id": table.id, "event": "change", "value": 1})
        app._dispatch({"type": "event", "id": table.id, "event": "change", "value": None})
        app._dispatch({"type": "event", "id": table.id, "event": "doubleClick", "value": 0})
        app._dispatch({"type": "event", "id": table.id, "event": "sort", "value": {"column": "name", "direction": "desc"}})
        app._dispatch({"type": "event", "id": tabs.id, "event": "change", "value": "table"})
        app.close()
        self.assertEqual(errors, [])
        self.assertEqual(
            seen,
            [("row", 1), ("row", None), ("open", 0), ("sort", {"column": "name", "direction": "desc"}), ("tab", "table")],
        )

    def test_a_tabs_of_tab_nodes(self):
        app = App(headless=True)
        errors, seen = [], []
        app.on_error = errors.append
        tabs = app.tabs(value="files", variant="underline")
        tabs.on("change", lambda value: seen.append(("tab", value)))
        files = app.tab(value="files", a11y={"label": "Files"})
        files.append(app.icon(name="folder", class_="size-4"), app.text(content="Files"), app.badge(count=3))
        tabs.append(files, app.tab(value="history", label="History", disabled=True))
        app.window(root=app.div().append(tabs))
        app.commit()

        app._dispatch({"type": "event", "id": tabs.id, "event": "change", "value": "files"})
        app.close()
        self.assertEqual(errors, [])
        self.assertEqual(seen, [("tab", "files")])

    def test_tree_diff_and_carousel_are_accepted_and_dispatch_events(self):
        app = App(headless=True)
        errors, seen = [], []
        app.on_error = errors.append

        tree = app.tree(
            items=[
                {"id": "src", "label": "src", "expanded": True, "children": [{"id": "src/lib.rs", "label": "lib.rs"}]},
                {"id": "README.md", "label": "README.md"},
            ],
            value="src",
        )
        tree.on("change", lambda value: seen.append(("change", value)))
        tree.on("expand", lambda value: seen.append(("expand", value)))
        tree.on("collapse", lambda value: seen.append(("collapse", value)))
        patch = (
            "diff --git a/notes.txt b/notes.txt\n"
            "--- a/notes.txt\n"
            "+++ b/notes.txt\n"
            "@@ -1,3 +1,4 @@\n"
            " one\n"
            "-two\n"
            "+2\n"
            " three\n"
            "+four\n"
        )
        diff = app.diff(patch=patch, mode="split")
        diff.on("change", lambda selection: seen.append(("diff", selection)))
        carousel = app.carousel(value=0, pagination=True, class_="h-40")
        carousel.append(app.text(content="One"), app.text(content="Two"))
        carousel.on("change", lambda index: seen.append(("slide", index)))
        app.window(root=app.div().append(tree, diff, carousel))
        app.commit()

        selection = {"path": "notes.txt", "side": "modified", "start": 1, "end": 2, "endSide": "modified"}
        app._dispatch({"type": "event", "id": tree.id, "event": "change", "value": "README.md"})
        app._dispatch({"type": "event", "id": tree.id, "event": "expand", "value": "src"})
        app._dispatch({"type": "event", "id": tree.id, "event": "collapse", "value": "src"})
        app._dispatch({"type": "event", "id": diff.id, "event": "change", "value": selection})
        app._dispatch({"type": "event", "id": diff.id, "event": "change", "value": None})
        app._dispatch({"type": "event", "id": carousel.id, "event": "change", "value": 1})
        app.close()
        self.assertEqual(errors, [])
        self.assertEqual(
            seen,
            [
                ("change", "README.md"),
                ("expand", "src"),
                ("collapse", "src"),
                ("diff", selection),
                ("diff", None),
                ("slide", 1),
            ],
        )

    def test_sidebar_and_chart_are_accepted_and_the_sidebar_dispatches_events(self):
        app = App(headless=True)
        errors, seen = [], []
        app.on_error = errors.append

        sidebar = app.sidebar(
            items=[
                {
                    "label": "General",
                    "items": [
                        {"id": "home", "label": "Home", "icon": "layout-dashboard"},
                        {"id": "inbox", "label": "Inbox", "icon": "inbox"},
                    ],
                },
                {"items": [{"id": "settings", "label": "Settings", "children": [{"id": "theme", "label": "Theme"}]}]},
            ],
            value="home",
        )
        sidebar.on("change", lambda entry: seen.append(("sidebar", entry)))
        line = app.chart(
            chart="line",
            data=[{"label": "Mon", "value": 12}, {"label": "Tue", "value": 18}],
            yAxis=True,
        )
        pie = app.chart(chart="pie", data=[{"label": "Rust", "value": 60}], innerRadius=20)
        app.window(root=app.div().append(sidebar, line, pie))
        app.commit()

        app._dispatch({"type": "event", "id": sidebar.id, "event": "change", "value": "theme"})
        app.close()
        self.assertEqual(errors, [])
        self.assertEqual(seen, [("sidebar", "theme")])

    def test_malformed_sidebar_and_chart_props_are_invalid_props(self):
        app = App(headless=True)
        errors = []
        app.on_error = errors.append

        app.sidebar(items=[{"items": [{"id": "home"}]}])
        app.chart(data=[{"label": "Mon"}])
        app.window(root=app.div())
        app.commit()
        app.close()
        self.assertEqual([error.code for error in errors], ["invalid_props", "invalid_props"])

    def test_create_window_has_its_own_id_root_and_closed_callback(self):
        app = App(headless=True)
        errors, closed = [], []
        app.on_error = errors.append
        main, other = app.div(), app.div()
        app.window(title="Main", root=main)
        settings = app.create_window(title="Settings", root=other)
        third = app.create_window()
        self.assertEqual((settings.id, third.id), (2, 3))
        third.set(root=main)  # a node can be the root of one window only
        app.commit()

        settings.on_closed = lambda: closed.append(settings.id)
        app.on_window_closed = lambda window_id: closed.append(window_id * 10)
        app._dispatch({"type": "windowClosed", "id": 2})
        third.close()
        app.commit()

        app.close()
        self.assertEqual(closed, [2, 20])
        self.assertEqual([(e.code, e.op) for e in errors], [("invalid_operation", "window")])

    def test_window_events_are_opt_in_and_dispatched(self):
        app = App(headless=True)
        errors = []
        app.on_error = errors.append

        window = app.create_window(root=app.div())
        resized, seen = [], []
        window.on("resize", resized.append)
        app.on_window_event = seen.append
        # The subscription travels as `window.events`, and replaces the previous list.
        self.assertEqual(app._pending[-1]["events"], ["resize"])
        with self.assertRaises(ValueError):
            window.on("hover", print)
        app.commit()

        app._dispatch({"type": "windowEvent", "id": window.id, "event": "resize", "width": 500, "height": 400})
        self.assertEqual([message["width"] for message in resized], [500])
        self.assertEqual([message["event"] for message in seen], ["resize"])

        # The last unsubscribe sends an empty list, so the host stops sending them.
        window.off("resize")
        self.assertEqual(app._pending[-1]["events"], [])
        app.commit()

        app.close()
        self.assertEqual(errors, [])

    def test_host_crash_is_reported_clearly(self):
        app = App(headless=True)
        app._proc.kill()
        self.assertNotEqual(app.run().code, 0)
        with self.assertRaisesRegex(HostExitedError, "host exited"):
            app.commit()

    def test_missing_host_binary_fails_with_helpful_message(self):
        with self.assertRaisesRegex(HostExitedError, r"host not found at does-not-exist\.exe"):
            App("does-not-exist.exe")

    def test_default_host_path_prefers_the_packaged_asset(self):
        # The wheel is platform-tagged and carries its binary; a checkout has none staged,
        # so this checks the order: the explicit variable, the asset, the repository build.
        name = "gpui-bridge-host.exe" if sys.platform == "win32" else "gpui-bridge-host"
        packaged = Path(gpui_bridge.__file__).resolve().parent / "bin" / name
        saved = os.environ.pop("GPUI_BRIDGE_HOST", None)
        try:
            if packaged.exists():
                self.assertEqual(gpui_bridge.default_host_path(), str(packaged))
            else:
                self.assertRegex(gpui_bridge.default_host_path(), r"target[\\/](debug|release)[\\/]gpui-bridge-host")
            os.environ["GPUI_BRIDGE_HOST"] = "an-explicit-host"
            self.assertEqual(gpui_bridge.default_host_path(), "an-explicit-host")
        finally:
            os.environ.pop("GPUI_BRIDGE_HOST", None)
            if saved is not None:
                os.environ["GPUI_BRIDGE_HOST"] = saved


if __name__ == "__main__":
    unittest.main()
