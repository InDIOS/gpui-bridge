# Runs against the real host binary in headless mode (build it first: `cargo build` in host/).
# Run: python -m unittest   (from client-python/)

import unittest

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


if __name__ == "__main__":
    unittest.main()
