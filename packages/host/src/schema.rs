//! The node types and their props (PROTOCOL.md section 5): what `create` and `setProps` accept.

use serde_json::Value;

use crate::protocol::Props;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PropKind {
    Str,
    Bool,
    OneOf(&'static [&'static str]),
    Icon,
    /// A menu item list (PROTOCOL.md section 10).
    Menu,
    /// A `MenuBar`'s list of `{label, items}`.
    Menus,
    /// A finite number.
    Number,
    /// A finite number from 0.
    NonNegative,
    /// A finite number above 0.
    Positive,
    /// Standard base64 (an `Image`'s bytes); the value is decoded when the op is applied, so a
    /// bad payload never reaches the error message.
    Base64,
    /// A RadioGroup's or Select's list of `{value, label, disabled?}` (also a Tabs' `tabs`).
    Options,
    /// A whole number from 0 (a Table row).
    Index,
    /// A Table's `columns` (PROTOCOL.md section 15).
    Columns,
    /// A Table's `rows`.
    Rows,
    /// The `a11y` prop every node accepts (PROTOCOL.md section 16).
    Accessibility,
    /// An Accordion's `items`: `{id, title, content?}` (PROTOCOL.md section 17).
    Items,
    /// A DescriptionList's `items`: `{label, value, span?}`.
    Entries,
    /// A Resizable's `sizes`: one initial size in pixels per child.
    Sizes,
    /// A Tree's `items`: `{id, label, children?, expanded?, disabled?}`, recursively.
    TreeItems,
    /// A Diff's `patch`: a unified or Git diff gpui-component can parse.
    Patch,
    /// A Sidebar's `items`: sections of `{id, label, ...}` entries (PROTOCOL.md section 19).
    Sections,
    /// A Chart's `data`: `{label, value}` points.
    Series,
    /// A Canvas's `commands`: a list of drawing command objects (PROTOCOL.md section 20).
    Commands,
    /// A Command's `items`: sections of `{id, label, ...}` entries (PROTOCOL.md section 21).
    CommandSections,
    /// A Settings' `pages`: pages of groups of typed fields (PROTOCOL.md section 22).
    SettingsPages,
    /// A Questionnaire's `items`: questions with choices or free text (PROTOCOL.md section 26).
    Questions,
    /// A list of plain strings (`Combobox`'s `value`, `ColorPicker`'s `palette`).
    StringList,
}

impl PropKind {
    pub fn accepts(self, value: &Value) -> bool {
        match self {
            Self::Str => value.is_string(),
            Self::Bool => value.is_boolean(),
            Self::OneOf(allowed) => value.as_str().is_some_and(|v| allowed.contains(&v)),
            Self::Icon => value.as_str().is_some_and(crate::components::icon_exists),
            Self::Menu
            | Self::Menus
            | Self::Options
            | Self::Columns
            | Self::Rows
            | Self::Accessibility
            | Self::Items
            | Self::Entries
            | Self::Sizes
            | Self::TreeItems
            | Self::Patch
            | Self::Sections
            | Self::Series
            | Self::Commands
            | Self::CommandSections
            | Self::SettingsPages
            | Self::Questions => self.check(value).is_ok(),
            Self::StringList => value
                .as_array()
                .is_some_and(|arr| arr.iter().all(Value::is_string)),
            Self::Number => value.as_f64().is_some_and(f64::is_finite),
            Self::NonNegative => value.as_f64().is_some_and(|n| n.is_finite() && n >= 0.),
            Self::Positive => value.as_f64().is_some_and(|n| n.is_finite() && n > 0.),
            Self::Base64 => value.is_string(),
            Self::Index => value.is_u64(),
        }
    }

    /// The reason a value is rejected; menus say which item is wrong.
    pub fn check(self, value: &Value) -> Result<(), String> {
        match self {
            Self::Menu => crate::components::menu::check_items(value)
                .map_err(|err| format!("a menu item list ({err})")),
            Self::Menus => crate::components::menu::check_menus(value)
                .map_err(|err| format!("a list of menus ({err})")),
            Self::Options => crate::components::check_options(value)
                .map_err(|err| format!("a list of options ({err})")),
            Self::Columns => crate::components::table::check_columns(value)
                .map_err(|err| format!("a list of columns ({err})")),
            Self::Rows => crate::components::table::check_rows(value)
                .map_err(|err| format!("a list of rows ({err})")),
            Self::Accessibility => {
                crate::a11y::check(value).map_err(|err| format!("an a11y object ({err})"))
            }
            Self::Items => crate::components::accordion::check_items(value)
                .map_err(|err| format!("a list of accordion items ({err})")),
            Self::Entries => crate::components::layout::check_entries(value)
                .map_err(|err| format!("a list of entries ({err})")),
            Self::Sizes => crate::components::layout::check_sizes(value)
                .map_err(|err| format!("a list of sizes ({err})")),
            Self::TreeItems => crate::components::tree::check_items(value)
                .map_err(|err| format!("a list of tree items ({err})")),
            Self::Patch => crate::components::diff::check_patch(value)
                .map_err(|err| format!("a patch ({err})")),
            Self::Sections => crate::components::sidebar::check_items(value)
                .map_err(|err| format!("a list of sidebar sections ({err})")),
            Self::Series => crate::components::chart::check_data(value)
                .map_err(|err| format!("a list of data points ({err})")),
            Self::Commands => crate::components::canvas::check_commands(value)
                .map_err(|err| format!("a list of canvas commands ({err})")),
            Self::CommandSections => crate::components::command::check_items(value)
                .map_err(|err| format!("a list of command sections ({err})")),
            Self::Questions => crate::components::questionnaire::check_items(value)
                .map_err(|err| format!("a list of questions ({err})")),
            Self::SettingsPages => crate::components::settings::check_pages(value)
                .map_err(|err| format!("a list of settings pages ({err})")),
            _ if self.accepts(value) => Ok(()),
            _ => Err(self.describe()),
        }
    }

    fn describe(self) -> String {
        match self {
            Self::Str => "a string".into(),
            Self::Bool => "a boolean".into(),
            Self::OneOf(allowed) => format!("one of {}", allowed.join(", ")),
            Self::Icon => "a bundled icon name (PROTOCOL.md section 5)".into(),
            Self::Menu | Self::Menus => "a menu list (PROTOCOL.md section 10)".into(),
            Self::Options => "a list of options".into(),
            Self::Number => "a number".into(),
            Self::Positive => "a number above 0".into(),
            Self::NonNegative => "a number from 0".into(),
            Self::Base64 => "a base64-encoded string".into(),
            Self::Index => "a whole number from 0".into(),
            Self::Columns => "a list of columns".into(),
            Self::Rows => "a list of rows".into(),
            Self::Accessibility => "an a11y object (PROTOCOL.md section 16)".into(),
            Self::Items => "a list of accordion items".into(),
            Self::Entries => "a list of entries".into(),
            Self::Sizes => "a list of positive sizes".into(),
            Self::TreeItems => "a list of tree items".into(),
            Self::Patch => "a unified diff string".into(),
            Self::Sections => "a list of sidebar sections".into(),
            Self::Series => "a list of data points".into(),
            Self::Commands => "a list of canvas command objects".into(),
            Self::CommandSections => "a list of command sections".into(),
            Self::SettingsPages => "a list of settings pages".into(),
            Self::Questions => "a list of questions".into(),
            Self::StringList => "a list of strings".into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NodeType {
    Div,
    Text,
    Button,
    Input,
    Checkbox,
    Icon,
    Image,
    MenuBar,
    Switch,
    RadioGroup,
    Select,
    Slider,
    Textarea,
    Dialog,
    VirtualList,
    Table,
    Tabs,
    /// The chip of a `Tabs` built from nodes (PROTOCOL.md section 15).
    Tab,
    // Basic components (PROTOCOL.md section 5).
    Alert,
    Badge,
    Tag,
    Kbd,
    Progress,
    Spinner,
    Skeleton,
    Rating,
    Pagination,
    Collapsible,
    Accordion,
    Avatar,
    Stepper,
    // Form components.
    NumberInput,
    OtpInput,
    Form,
    Field,
    // Layout components.
    GroupBox,
    DescriptionList,
    StatusBar,
    Toolbar,
    Resizable,
    // Advanced components.
    Calendar,
    // Data components (PROTOCOL.md section 18).
    Tree,
    Diff,
    Carousel,
    // Data components (PROTOCOL.md section 19).
    Sidebar,
    Chart,
    // Canvas (PROTOCOL.md section 20).
    Canvas,
    // Command palette (PROTOCOL.md section 21).
    Command,
    // Settings pages (PROTOCOL.md section 22).
    Settings,
    // Simple components (PROTOCOL.md section 23).
    Toggle,
    DropdownButton,
    Label,
    Empty,
    Shimmer,
    InputGroup,
    Clipboard,
    // Overlays (PROTOCOL.md section 24).
    AlertDialog,
    Sheet,
    Popover,
    HoverCard,
    // Form components with state (PROTOCOL.md section 25).
    DatePicker,
    TimeField,
    ColorPicker,
    Combobox,
    // Chat (PROTOCOL.md section 26).
    Bubble,
    Message,
    MessageHeader,
    MessageFooter,
    Marker,
    Attachment,
    MessageScroller,
    Questionnaire,
    List,
    ListItem,
    // Dock (PROTOCOL.md section 28).
    Dock,
    DockPanel,
    // Slots as children (ADR 0004): drawn by the node that holds them.
    AccordionItem,
    DialogFooter,
    StatusBarLeft,
    StatusBarRight,
}

/// Where a `Popover` or `HoverCard` opens from its trigger (PROTOCOL.md section 24).
const ANCHORS: &[&str] = &[
    "top-left",
    "top-center",
    "top-right",
    "left-center",
    "right-center",
    "bottom-left",
    "bottom-center",
    "bottom-right",
];

/// The edge a `Sheet` slides in from.
const SIDES: &[&str] = &["left", "right", "top", "bottom"];

impl NodeType {
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "div" => Some(Self::Div),
            "text" => Some(Self::Text),
            "Button" => Some(Self::Button),
            "Input" => Some(Self::Input),
            "Checkbox" => Some(Self::Checkbox),
            "Icon" => Some(Self::Icon),
            "Image" => Some(Self::Image),
            "MenuBar" => Some(Self::MenuBar),
            "Switch" => Some(Self::Switch),
            "RadioGroup" => Some(Self::RadioGroup),
            "Select" => Some(Self::Select),
            "Slider" => Some(Self::Slider),
            "Textarea" => Some(Self::Textarea),
            "Dialog" => Some(Self::Dialog),
            "VirtualList" => Some(Self::VirtualList),
            "Table" => Some(Self::Table),
            "Tabs" => Some(Self::Tabs),
            "Tab" => Some(Self::Tab),
            "Alert" => Some(Self::Alert),
            "Badge" => Some(Self::Badge),
            "Tag" => Some(Self::Tag),
            "Kbd" => Some(Self::Kbd),
            "Progress" => Some(Self::Progress),
            "Spinner" => Some(Self::Spinner),
            "Skeleton" => Some(Self::Skeleton),
            "Rating" => Some(Self::Rating),
            "Pagination" => Some(Self::Pagination),
            "Collapsible" => Some(Self::Collapsible),
            "Accordion" => Some(Self::Accordion),
            "Avatar" => Some(Self::Avatar),
            "Stepper" => Some(Self::Stepper),
            "NumberInput" => Some(Self::NumberInput),
            "OtpInput" => Some(Self::OtpInput),
            "Form" => Some(Self::Form),
            "Field" => Some(Self::Field),
            "GroupBox" => Some(Self::GroupBox),
            "DescriptionList" => Some(Self::DescriptionList),
            "StatusBar" => Some(Self::StatusBar),
            "Toolbar" => Some(Self::Toolbar),
            "Resizable" => Some(Self::Resizable),
            "Calendar" => Some(Self::Calendar),
            "Tree" => Some(Self::Tree),
            "Diff" => Some(Self::Diff),
            "Carousel" => Some(Self::Carousel),
            "Sidebar" => Some(Self::Sidebar),
            "Chart" => Some(Self::Chart),
            "Canvas" => Some(Self::Canvas),
            "Command" => Some(Self::Command),
            "Settings" => Some(Self::Settings),
            "Toggle" => Some(Self::Toggle),
            "DropdownButton" => Some(Self::DropdownButton),
            "Label" => Some(Self::Label),
            "Empty" => Some(Self::Empty),
            "Shimmer" => Some(Self::Shimmer),
            "InputGroup" => Some(Self::InputGroup),
            "Clipboard" => Some(Self::Clipboard),
            "AlertDialog" => Some(Self::AlertDialog),
            "Sheet" => Some(Self::Sheet),
            "Popover" => Some(Self::Popover),
            "HoverCard" => Some(Self::HoverCard),
            "DatePicker" => Some(Self::DatePicker),
            "TimeField" => Some(Self::TimeField),
            "ColorPicker" => Some(Self::ColorPicker),
            "Combobox" => Some(Self::Combobox),
            "Bubble" => Some(Self::Bubble),
            "Message" => Some(Self::Message),
            "MessageHeader" => Some(Self::MessageHeader),
            "MessageFooter" => Some(Self::MessageFooter),
            "Marker" => Some(Self::Marker),
            "Attachment" => Some(Self::Attachment),
            "MessageScroller" => Some(Self::MessageScroller),
            "Questionnaire" => Some(Self::Questionnaire),
            "List" => Some(Self::List),
            "ListItem" => Some(Self::ListItem),
            "Dock" => Some(Self::Dock),
            "DockPanel" => Some(Self::DockPanel),
            "AccordionItem" => Some(Self::AccordionItem),
            "DialogFooter" => Some(Self::DialogFooter),
            "StatusBarLeft" => Some(Self::StatusBarLeft),
            "StatusBarRight" => Some(Self::StatusBarRight),
            _ => None,
        }
    }

    /// Expected kind of each known prop (PROTOCOL.md section 5); `None` for unknown props.
    pub fn prop_kind(self, key: &str) -> Option<PropKind> {
        use PropKind::*;
        match (self, key) {
            (_, "class") => Some(Str),
            (_, "a11y") => Some(Accessibility),
            (Self::Text, "content") => Some(Str),
            (Self::Button, "label") => Some(Str),
            (Self::Button | Self::DropdownButton, "variant") => {
                Some(OneOf(&["primary", "secondary", "ghost", "danger"]))
            }
            (Self::Input | Self::Textarea, "value" | "placeholder") => Some(Str),
            (Self::Checkbox | Self::Switch | Self::Toggle, "label") => Some(Str),
            (Self::Checkbox | Self::Switch | Self::Toggle, "checked") => Some(Bool),
            (Self::RadioGroup | Self::Select, "options") => Some(Options),
            (Self::RadioGroup | Self::Select, "value") => Some(Str),
            (Self::RadioGroup, "direction") => Some(OneOf(&["vertical", "horizontal"])),
            (Self::Select, "placeholder") => Some(Str),
            (Self::Select, "searchable") => Some(Bool),
            (Self::Slider, "value" | "min" | "max") => Some(Number),
            (Self::Slider, "step") => Some(Positive),
            (
                Self::Div | Self::Button | Self::Toggle | Self::DropdownButton | Self::Clipboard,
                "tooltip",
            ) => Some(Str),
            (Self::Dialog | Self::AlertDialog, "open") => Some(Bool),
            (Self::Dialog, "title" | "okText" | "cancelText") => Some(Str),
            (Self::Dialog, "width") => Some(Positive),
            (Self::AlertDialog, "title" | "description" | "okText" | "cancelText") => Some(Str),
            (Self::AlertDialog, "icon") => Some(Icon),
            (Self::Sheet, "open") => Some(Bool),
            (Self::Sheet, "side") => Some(OneOf(SIDES)),
            (Self::Sheet, "title") => Some(Str),
            (Self::Sheet, "size") => Some(Positive),
            (Self::Popover, "open" | "arrow") => Some(Bool),
            (Self::Popover | Self::HoverCard, "anchor") => Some(OneOf(ANCHORS)),
            (Self::HoverCard, "openDelay" | "closeDelay") => Some(NonNegative),
            (Self::VirtualList, "rowHeight") => Some(Positive),
            (Self::Table, "columns") => Some(Columns),
            (Self::Table, "rows") => Some(Rows),
            (Self::Table, "selected") => Some(Index),
            (Self::Table, "stripe" | "bordered") => Some(Bool),
            (Self::Tabs, "tabs") => Some(Options),
            (Self::Tabs, "value") => Some(Str),
            (Self::Tabs, "variant") => {
                Some(OneOf(&["tab", "outline", "pill", "segmented", "underline"]))
            }
            (Self::Tab, "value" | "label") => Some(Str),
            (Self::Tab, "disabled") => Some(Bool),
            (Self::Icon, "name") => Some(Icon),
            (Self::Div, "windowControl") => Some(OneOf(&["drag", "minimize", "maximize", "close"])),
            (Self::Image, "src") => Some(Str),
            (Self::Image, "data") => Some(Base64),
            (Self::Div, "contextMenu") => Some(Menu),
            (Self::MenuBar, "menus") => Some(Menus),
            (Self::Image, "fit") => {
                Some(OneOf(&["contain", "cover", "fill", "scale-down", "none"]))
            }
            // Basic components (PROTOCOL.md section 17).
            (Self::Alert, "message" | "title") => Some(Str),
            (Self::Alert, "variant") => {
                Some(OneOf(&["default", "info", "success", "warning", "error"]))
            }
            (Self::Alert, "banner" | "closable") => Some(Bool),
            (Self::Badge, "count" | "max") => Some(Index),
            (Self::Badge, "dot") => Some(Bool),
            (Self::Badge | Self::Tag, "icon") => Some(Icon),
            (Self::Tag, "label") => Some(Str),
            (Self::Tag, "variant") => Some(OneOf(&[
                "primary",
                "secondary",
                "danger",
                "success",
                "warning",
                "info",
            ])),
            (Self::Tag | Self::Kbd, "outline") => Some(Bool),
            (Self::Kbd, "keys") => Some(Str),
            (Self::Progress, "value") => Some(Number),
            (Self::Progress, "loading") => Some(Bool),
            (Self::Skeleton, "secondary") => Some(Bool),
            (Self::Rating, "value" | "max") => Some(Index),
            (Self::Pagination, "page" | "total") => Some(Index),
            (Self::Pagination, "compact") => Some(Bool),
            (Self::Collapsible, "open") => Some(Bool),
            (Self::Accordion, "items") => Some(Items),
            (Self::Accordion, "multiple" | "bordered") => Some(Bool),
            (Self::AccordionItem, "id" | "title") => Some(Str),
            (Self::AccordionItem, "open" | "disabled") => Some(Bool),
            (Self::Avatar, "name" | "src") => Some(Str),
            (Self::Stepper, "items") => Some(Options),
            (Self::Stepper, "value") => Some(Str),
            (Self::Stepper, "vertical") => Some(Bool),
            // Form components.
            (Self::NumberInput, "value" | "placeholder") => Some(Str),
            (Self::OtpInput, "length") => Some(Index),
            (Self::OtpInput, "value") => Some(Str),
            (Self::OtpInput, "masked") => Some(Bool),
            (Self::Form, "direction") => Some(OneOf(&["vertical", "horizontal"])),
            (Self::Field, "label" | "description") => Some(Str),
            (Self::Field, "required") => Some(Bool),
            // Form components with state (PROTOCOL.md section 25).
            (Self::DatePicker, "value" | "placeholder") => Some(Str),
            (Self::DatePicker, "range" | "disabled" | "controlled") => Some(Bool),
            (Self::TimeField, "value") => Some(Str),
            (Self::TimeField, "seconds" | "disabled" | "controlled") => Some(Bool),
            (Self::ColorPicker, "value") => Some(Str),
            (Self::ColorPicker, "palette") => Some(StringList),
            // No `disabled`: neither of the crate's color pickers can be disabled.
            (Self::ColorPicker, "controlled") => Some(Bool),
            (Self::Combobox, "options") => Some(Options),
            (Self::Combobox, "value") => Some(StringList),
            (Self::Combobox, "placeholder") => Some(Str),
            (Self::Combobox, "searchable" | "disabled" | "controlled") => Some(Bool),
            // Layout components.
            (Self::GroupBox, "title") => Some(Str),
            (Self::GroupBox, "variant") => Some(OneOf(&["normal", "fill", "outline"])),
            (Self::DescriptionList, "items") => Some(Entries),
            (Self::DescriptionList, "columns") => Some(Positive),
            (Self::DescriptionList, "bordered" | "horizontal") => Some(Bool),
            (Self::StatusBar, "left" | "right") => Some(Str),
            (Self::Resizable, "axis") => Some(OneOf(&["horizontal", "vertical"])),
            (Self::Resizable, "sizes") => Some(Sizes),
            // Advanced components.
            (Self::Calendar, "value") => Some(Str),
            (Self::Calendar, "months") => Some(Positive),
            (Self::Calendar, "weekStartsOn") => Some(OneOf(&["monday", "sunday"])),
            // Data components (PROTOCOL.md section 18).
            (Self::Tree, "items") => Some(TreeItems),
            (Self::Tree, "value") => Some(Str),
            (Self::Diff, "patch") => Some(Patch),
            (Self::Diff, "mode") => Some(OneOf(&["unified", "split"])),
            (Self::Diff, "lineNumbers" | "softWrap" | "header") => Some(Bool),
            (Self::Carousel, "value") => Some(Index),
            (Self::Carousel, "axis") => Some(OneOf(&["horizontal", "vertical"])),
            (Self::Carousel, "looping" | "controls" | "pagination") => Some(Bool),
            // Data components (PROTOCOL.md section 19).
            (Self::Sidebar, "items") => Some(Sections),
            (Self::Sidebar, "value") => Some(Str),
            (Self::Sidebar, "collapsible") => Some(OneOf(&["icon", "offcanvas", "none"])),
            (Self::Sidebar, "side") => Some(OneOf(&["left", "right"])),
            (Self::Sidebar, "collapsed") => Some(Bool),
            (Self::Chart, "data") => Some(Series),
            (Self::Chart, "chart") => Some(OneOf(&["line", "bar", "pie"])),
            (Self::Chart, "color") => Some(OneOf(crate::components::chart::COLORS)),
            (Self::Chart, "name") => Some(Str),
            (Self::Chart, "curve") => Some(OneOf(&["linear", "natural", "step"])),
            (Self::Chart, "dot" | "horizontal" | "grid" | "xAxis" | "yAxis" | "interactive") => {
                Some(Bool)
            }
            (Self::Chart, "yMin" | "yMax" | "innerRadius" | "padAngle") => Some(Number),
            (Self::Chart, "xTicks" | "yTicks") => Some(Index),
            (Self::Canvas, "commands") => Some(Commands),
            (Self::Command, "items") => Some(CommandSections),
            (Self::Command, "placeholder") => Some(Str),
            (Self::Settings, "pages") => Some(SettingsPages),
            // Simple components (PROTOCOL.md section 23).
            (Self::Toggle | Self::DropdownButton | Self::Empty, "icon") => Some(Icon),
            (Self::Toggle, "variant") => Some(OneOf(&["ghost", "outline"])),
            (Self::DropdownButton, "items") => Some(Menu),
            (Self::DropdownButton, "outline") => Some(Bool),
            (Self::DropdownButton, "menuLabel") => Some(Str),
            (Self::Label, "content" | "secondary" | "highlight") => Some(Str),
            (Self::Label, "masked") => Some(Bool),
            (Self::Empty, "title" | "description") => Some(Str),
            (Self::Shimmer, "content") => Some(Str),
            (Self::Shimmer, "duration" | "spread") => Some(Number),
            (Self::Shimmer, "reverse" | "once") => Some(Bool),
            (Self::InputGroup, "value" | "placeholder" | "prefix" | "suffix") => Some(Str),
            (Self::InputGroup, "prefixIcon" | "suffixIcon") => Some(Icon),
            (Self::InputGroup, "readonly" | "invalid") => Some(Bool),
            (Self::Clipboard, "value") => Some(Str),
            (Self::Bubble, "variant") => Some(OneOf(&[
                "filled",
                "secondary",
                "muted",
                "tinted",
                "outline",
                "ghost",
                "destructive",
            ])),
            (Self::Bubble | Self::Message, "alignment") => Some(OneOf(&["start", "end"])),
            (Self::Message, "avatarName" | "avatarSrc") => Some(Str),
            (Self::Marker, "content") => Some(Str),
            (Self::Marker, "icon") => Some(Icon),
            (Self::Marker, "variant") => Some(OneOf(&["plain", "separator", "border"])),
            (Self::Marker, "alignment") => Some(OneOf(&["start", "center", "end"])),
            (Self::Marker, "loading") => Some(Bool),
            (Self::Marker, "loadingStyle") => Some(OneOf(&["spinner", "shimmer"])),
            (Self::Attachment, "status") => Some(OneOf(&[
                "pending",
                "uploading",
                "processing",
                "failed",
                "complete",
            ])),
            (Self::Attachment, "progress") => Some(Number),
            (Self::Attachment, "title" | "description" | "tooltip" | "src") => Some(Str),
            (Self::Attachment, "data") => Some(Base64),
            (Self::Attachment, "axis") => Some(OneOf(&["horizontal", "vertical"])),
            (Self::MessageScroller, "scrollbar" | "jumpButton") => Some(Bool),
            (Self::MessageScroller, "jumpLabel") => Some(Str),
            (Self::Questionnaire, "items") => Some(Questions),
            (Self::List, "value" | "placeholder") => Some(Str),
            (Self::List, "searchable" | "hasMore" | "loading") => Some(Bool),
            (Self::List, "loadMoreThreshold") => Some(Index),
            (Self::ListItem, "value") => Some(Str),
            (Self::ListItem, "disabled") => Some(Bool),
            (Self::DockPanel, "title") => Some(Str),
            (Self::DockPanel, "placement") => Some(OneOf(&["center", "left", "right", "bottom"])),
            (Self::DockPanel, "closable") => Some(Bool),
            (
                Self::Input
                | Self::Checkbox
                | Self::Switch
                | Self::RadioGroup
                | Self::Select
                | Self::Slider
                | Self::Table
                | Self::Tabs
                | Self::Rating
                | Self::Pagination
                | Self::Stepper
                | Self::NumberInput
                | Self::OtpInput
                | Self::Calendar
                | Self::Tree
                | Self::Carousel
                | Self::Sidebar
                | Self::List
                | Self::Toggle
                | Self::InputGroup,
                "controlled",
            ) => Some(Bool),
            (
                Self::Button
                | Self::Input
                | Self::Checkbox
                | Self::Switch
                | Self::RadioGroup
                | Self::Select
                | Self::Slider
                | Self::Textarea
                | Self::Rating
                | Self::Pagination
                | Self::Accordion
                | Self::Stepper
                | Self::NumberInput
                | Self::OtpInput
                | Self::Calendar
                | Self::Toggle
                | Self::DropdownButton
                | Self::InputGroup,
                "disabled",
            ) => Some(Bool),
            _ => None,
        }
    }

    /// The types whose `src`/`data` is a picture, kept by the tree like an `Image`'s.
    pub fn takes_image(self) -> bool {
        matches!(self, Self::Image | Self::Attachment)
    }

    /// The props a node cannot be built without: the `value` a `Tab` and a `ListItem` report and
    /// the `id` an `AccordionItem` reports (PROTOCOL.md sections 15, 27 and 17). `props` is the
    /// whole message, so this also rejects a `setProps` that removes or empties it.
    pub fn check_required(self, props: &Props) -> Result<(), String> {
        let key = match self {
            Self::Tab | Self::ListItem => "value",
            Self::AccordionItem => "id",
            _ => return Ok(()),
        };
        if props
            .get(key)
            .and_then(Value::as_str)
            .is_some_and(|value| !value.is_empty())
        {
            return Ok(());
        }
        Err(format!(
            "A {} needs a non-empty '{key}' string",
            self.name()
        ))
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Div => "div",
            Self::Text => "text",
            Self::Button => "Button",
            Self::Input => "Input",
            Self::Checkbox => "Checkbox",
            Self::Icon => "Icon",
            Self::Image => "Image",
            Self::MenuBar => "MenuBar",
            Self::Switch => "Switch",
            Self::RadioGroup => "RadioGroup",
            Self::Select => "Select",
            Self::Slider => "Slider",
            Self::Textarea => "Textarea",
            Self::Dialog => "Dialog",
            Self::VirtualList => "VirtualList",
            Self::Table => "Table",
            Self::Tabs => "Tabs",
            Self::Tab => "Tab",
            Self::Alert => "Alert",
            Self::Badge => "Badge",
            Self::Tag => "Tag",
            Self::Kbd => "Kbd",
            Self::Progress => "Progress",
            Self::Spinner => "Spinner",
            Self::Skeleton => "Skeleton",
            Self::Rating => "Rating",
            Self::Pagination => "Pagination",
            Self::Collapsible => "Collapsible",
            Self::Accordion => "Accordion",
            Self::Avatar => "Avatar",
            Self::Stepper => "Stepper",
            Self::NumberInput => "NumberInput",
            Self::OtpInput => "OtpInput",
            Self::Form => "Form",
            Self::Field => "Field",
            Self::GroupBox => "GroupBox",
            Self::DescriptionList => "DescriptionList",
            Self::StatusBar => "StatusBar",
            Self::Toolbar => "Toolbar",
            Self::Resizable => "Resizable",
            Self::Calendar => "Calendar",
            Self::Tree => "Tree",
            Self::Diff => "Diff",
            Self::Carousel => "Carousel",
            Self::Sidebar => "Sidebar",
            Self::Chart => "Chart",
            Self::Canvas => "Canvas",
            Self::Command => "Command",
            Self::Settings => "Settings",
            Self::Toggle => "Toggle",
            Self::DropdownButton => "DropdownButton",
            Self::Label => "Label",
            Self::Empty => "Empty",
            Self::Shimmer => "Shimmer",
            Self::InputGroup => "InputGroup",
            Self::Clipboard => "Clipboard",
            Self::AlertDialog => "AlertDialog",
            Self::Sheet => "Sheet",
            Self::Popover => "Popover",
            Self::HoverCard => "HoverCard",
            Self::DatePicker => "DatePicker",
            Self::TimeField => "TimeField",
            Self::ColorPicker => "ColorPicker",
            Self::Combobox => "Combobox",
            Self::Bubble => "Bubble",
            Self::Message => "Message",
            Self::MessageHeader => "MessageHeader",
            Self::MessageFooter => "MessageFooter",
            Self::Marker => "Marker",
            Self::Attachment => "Attachment",
            Self::MessageScroller => "MessageScroller",
            Self::Questionnaire => "Questionnaire",
            Self::List => "List",
            Self::ListItem => "ListItem",
            Self::Dock => "Dock",
            Self::DockPanel => "DockPanel",
            Self::AccordionItem => "AccordionItem",
            Self::DialogFooter => "DialogFooter",
            Self::StatusBarLeft => "StatusBarLeft",
            Self::StatusBarRight => "StatusBarRight",
        }
    }
}

/// Every node type, for the tests that keep the list, the names and the docs in step.
#[cfg(test)]
pub const ALL: &[NodeType] = &[
    NodeType::Div,
    NodeType::Text,
    NodeType::Button,
    NodeType::Input,
    NodeType::Checkbox,
    NodeType::Icon,
    NodeType::Image,
    NodeType::MenuBar,
    NodeType::Switch,
    NodeType::RadioGroup,
    NodeType::Select,
    NodeType::Slider,
    NodeType::Textarea,
    NodeType::Dialog,
    NodeType::VirtualList,
    NodeType::Table,
    NodeType::Tabs,
    NodeType::Tab,
    NodeType::Alert,
    NodeType::Badge,
    NodeType::Tag,
    NodeType::Kbd,
    NodeType::Progress,
    NodeType::Spinner,
    NodeType::Skeleton,
    NodeType::Rating,
    NodeType::Pagination,
    NodeType::Collapsible,
    NodeType::Accordion,
    NodeType::Avatar,
    NodeType::Stepper,
    NodeType::NumberInput,
    NodeType::OtpInput,
    NodeType::Form,
    NodeType::Field,
    NodeType::GroupBox,
    NodeType::DescriptionList,
    NodeType::StatusBar,
    NodeType::Toolbar,
    NodeType::Resizable,
    NodeType::Calendar,
    NodeType::Tree,
    NodeType::Diff,
    NodeType::Carousel,
    NodeType::Sidebar,
    NodeType::Chart,
    NodeType::Canvas,
    NodeType::Command,
    NodeType::Settings,
    NodeType::Toggle,
    NodeType::DropdownButton,
    NodeType::Label,
    NodeType::Empty,
    NodeType::Shimmer,
    NodeType::InputGroup,
    NodeType::Clipboard,
    NodeType::AlertDialog,
    NodeType::Sheet,
    NodeType::Popover,
    NodeType::HoverCard,
    NodeType::DatePicker,
    NodeType::TimeField,
    NodeType::ColorPicker,
    NodeType::Combobox,
    NodeType::Bubble,
    NodeType::Message,
    NodeType::MessageHeader,
    NodeType::MessageFooter,
    NodeType::Marker,
    NodeType::Attachment,
    NodeType::MessageScroller,
    NodeType::Questionnaire,
    NodeType::List,
    NodeType::ListItem,
    NodeType::Dock,
    NodeType::DockPanel,
    NodeType::AccordionItem,
    NodeType::DialogFooter,
    NodeType::StatusBarLeft,
    NodeType::StatusBarRight,
];

#[cfg(test)]
mod tests {
    // Explicit imports: `super::*` would bring GPUI's `test` macro, shadowing Rust's.
    use super::{ALL, NodeType, PropKind};
    use serde_json::json;

    #[test]
    fn every_type_round_trips_its_name() {
        for node_type in ALL {
            assert_eq!(
                NodeType::parse(node_type.name()),
                Some(*node_type),
                "{}",
                node_type.name()
            );
        }
    }

    #[test]
    fn every_type_takes_a11y() {
        for node_type in ALL {
            assert_eq!(
                node_type.prop_kind("a11y"),
                Some(PropKind::Accessibility),
                "{}",
                node_type.name()
            );
            assert!(
                node_type
                    .prop_kind("a11y")
                    .unwrap()
                    .accepts(&json!({ "role": "button", "label": "Go" }))
            );
        }
    }

    #[test]
    fn unknown_props_of_a_type_are_ignored() {
        assert_eq!(NodeType::Alert.prop_kind("label"), None);
        assert_eq!(NodeType::Spinner.prop_kind("value"), None);
    }
}
