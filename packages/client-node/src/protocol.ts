// Protocol message types. Source of truth together with `host/src/protocol.rs`
// and PROTOCOL.md.

export const PROTOCOL_VERSION = 1;

export type NodeId = number;
/** Client-assigned window id; `window` without `id` is window 1. */
export type WindowId = number;
export type NodeType =
  | "div"
  | "text"
  | "Button"
  | "Input"
  | "Checkbox"
  | "Icon"
  | "Image"
  | "MenuBar"
  | "Switch"
  | "RadioGroup"
  | "Select"
  | "Slider"
  | "Textarea"
  | "Dialog"
  | "VirtualList"
  | "Table"
  | "Tabs"
  | "Tab"
  | "Alert"
  | "Badge"
  | "Tag"
  | "Kbd"
  | "Progress"
  | "Spinner"
  | "Skeleton"
  | "Rating"
  | "Pagination"
  | "Collapsible"
  | "Accordion"
  | "Avatar"
  | "Stepper"
  | "NumberInput"
  | "OtpInput"
  | "Form"
  | "Field"
  | "GroupBox"
  | "DescriptionList"
  | "StatusBar"
  | "Toolbar"
  | "Resizable"
  | "Calendar"
  | "Tree"
  | "Diff"
  | "Carousel"
  | "Sidebar"
  | "Chart";
export type WindowTheme = "light" | "dark";

/** Events a window can report (PROTOCOL.md section 3); `window.events` opts in. */
export type WindowEventName = "focus" | "blur" | "resize" | "move" | "visibility";

/**
 * A gpui-component theme file (PROTOCOL.md section 3). Field names are the file's own
 * (snake_case, dotted color keys such as `"primary.background"`).
 */
export interface ThemeSet {
  name?: string;
  themes: ThemeConfig[];
}

export interface ThemeConfig {
  name?: string;
  mode?: WindowTheme;
  font_size?: number;
  font_family?: string;
  radius?: number;
  colors?: Record<string, string>;
  [field: string]: unknown;
}

/** Node props on the wire. In `setProps`, a `null` value removes the prop. */
export type Props = Record<string, unknown>;

/** Messages from the client (host stdin). */
export type ClientMessage =
  | { op: "hello"; protocol: number }
  | {
      op: "window";
      id?: WindowId;
      title?: string;
      width?: number;
      height?: number;
      /** Initial position; read when the window opens. */
      x?: number;
      y?: number;
      /** Minimum size the user can resize to; read when the window opens. */
      minWidth?: number;
      minHeight?: number;
      root?: NodeId;
      theme?: WindowTheme;
      themeSet?: ThemeSet;
      frameless?: boolean;
      /** Minimizes (`true`) or restores (`false`) on commit; an action, not a remembered state. */
      minimized?: boolean;
      /** Window events the client wants; replaces the previous list. */
      events?: WindowEventName[];
    }
  | { op: "create"; id: NodeId; type: NodeType; props?: Props; events?: string[] }
  | { op: "append"; parent: NodeId; child: NodeId }
  | { op: "insertBefore"; parent: NodeId; child: NodeId; before: NodeId }
  | { op: "setProps"; id: NodeId; props: Props }
  | { op: "setEvents"; id: NodeId; events: string[] }
  | { op: "remove"; id: NodeId }
  | { op: "closeWindow"; id: WindowId }
  | { op: "tray"; icon?: string; tooltip?: string; menu?: MenuItem[] }
  | { op: "removeTray" }
  | { op: "shortcuts"; keys: string[] }
  | { op: "focus"; id: NodeId }
  | { op: "openDialog"; request: number; files?: boolean; directories?: boolean; multiple?: boolean; prompt?: string }
  | { op: "saveDialog"; request: number; directory?: string; name?: string }
  | { op: "readClipboard"; request: number }
  | { op: "writeClipboard"; text: string }
  | { op: "openUrl"; url: string }
  | { op: "openPath"; path: string }
  | {
      op: "notify";
      window?: WindowId;
      id?: string;
      type?: "info" | "success" | "warning" | "error";
      title?: string;
      message: string;
      autohide?: boolean;
      system?: boolean;
    }
  | { op: "commit" };

export type ErrorCode =
  | "invalid_json"
  | "unknown_op"
  | "unknown_node"
  | "unknown_type"
  | "invalid_props"
  | "invalid_operation"
  | "protocol_mismatch";

/** Messages from the host (host stdout). */
export type HostMessage =
  | { type: "ready"; protocol: number }
  | { type: "event"; id: NodeId; event: string; value?: string | number | null | TableSort | DiffSelection; checked?: boolean; paths?: string[] }
  | { type: "windowClosed"; id: WindowId }
  /** A window event the client opted into with `window.events`; `move`/`resize` carry bounds. */
  | { type: "windowEvent"; id: WindowId; event: WindowEventName; x?: number; y?: number; width?: number; height?: number; visible?: boolean }
  | { type: "tray"; event: "click" | "menu"; value?: string }
  | { type: "shortcut"; keys: string; window: WindowId }
  | { type: "notification"; id: string; window: WindowId }
  | { type: "result"; request: number; paths?: string[] | null; path?: string | null; text?: string | null }
  | { type: "error"; code: ErrorCode; message: string; op?: string; request?: number };

// Props per node type (PROTOCOL.md section 5).

/** A window title bar part, handled by the system (PROTOCOL.md section 5). */
export type WindowControl = "drag" | "minimize" | "maximize" | "close";

/**
 * A menu entry (PROTOCOL.md section 10): an item with an `id`, a submenu with `items`, or a separator.
 * Choosing an item sends the node's `menu` event with its `id`.
 */
export type MenuItem =
  | {
      id: string;
      label: string;
      icon?: string;
      disabled?: boolean;
      checked?: boolean;
      /** A key such as `ctrl-s`: shown next to the label; in a `MenuBar` it also chooses the item. */
      shortcut?: string;
    }
  | { label: string; items: MenuItem[]; icon?: string; disabled?: boolean }
  | { separator: true };

/** A top-level menu of a `MenuBar`. */
export interface Menu {
  label: string;
  items: MenuItem[];
}

/** The roles a node may name (PROTOCOL.md section 16), in ARIA's names. */
export type A11yRole =
  | "alert" | "alertdialog" | "article" | "banner" | "button" | "cell" | "checkbox" | "columnheader"
  | "combobox" | "complementary" | "contentinfo" | "definition" | "dialog" | "document" | "form"
  | "grid" | "gridcell" | "group" | "heading" | "img" | "label" | "link" | "list" | "listbox"
  | "listitem" | "log" | "main" | "menu" | "menubar" | "menuitem" | "navigation" | "note"
  | "paragraph" | "progressbar" | "radio" | "radiogroup" | "region" | "row" | "rowheader"
  | "scrollbar" | "searchbox" | "separator" | "slider" | "spinbutton" | "status" | "switch"
  | "tab" | "table" | "tablist" | "tabpanel" | "textbox" | "time" | "toolbar" | "tooltip"
  | "tree" | "treeitem" | "window";

/**
 * Accessibility for a node (PROTOCOL.md section 16). GPUI reports to assistive technology only
 * the elements that have both an id and a role, so a `div` that acts as a button needs this.
 */
export interface A11y {
  /** A stable identifier, visible to assistive technology (UIA `AutomationId` on Windows). */
  id?: string;
  role?: A11yRole;
  label?: string;
  /** Supplementary text, read after the label. */
  description?: string;
  /** The keys that activate the element (`ctrl-s`). */
  keyshortcuts?: string;
  checked?: boolean | "mixed";
  selected?: boolean;
  expanded?: boolean;
  disabled?: boolean;
  /** Also reported as a number when `min`, `max` or `step` is given. */
  value?: string;
  min?: number;
  max?: number;
  step?: number;
  /** A heading's level, from 1. */
  level?: number;
  orientation?: "horizontal" | "vertical";
}

/** Props every node accepts (PROTOCOL.md section 16). */
export interface SharedProps {
  /** Nothing about the node is reported to assistive technology without it. */
  a11y?: A11y;
}

export interface DivProps {
  class?: string;
  windowControl?: WindowControl;
  /** Opens on right-click. */
  contextMenu?: MenuItem[];
  /** Shown when the pointer rests on the div. */
  tooltip?: string;
}

export interface TextProps {
  content?: string;
  class?: string;
}

export interface ButtonProps {
  label?: string;
  variant?: "primary" | "secondary" | "ghost" | "danger";
  disabled?: boolean;
  tooltip?: string;
  class?: string;
}

export interface InputProps {
  value?: string;
  placeholder?: string;
  disabled?: boolean;
  /** The host only reports edits; the client confirms them with `value`. */
  controlled?: boolean;
  class?: string;
}

export interface CheckboxProps {
  checked?: boolean;
  label?: string;
  disabled?: boolean;
  /** The host only reports clicks; the client confirms them with `checked`. */
  controlled?: boolean;
  class?: string;
}

export interface IconProps {
  /** A bundled icon, by its Lucide name (`folder`, `file-text`…; PROTOCOL.md section 5). */
  name?: string;
  class?: string;
}

export interface ImageProps {
  /** A local image file (PNG, JPG, GIF, WebP, SVG…); a relative path is resolved against the host's working directory. */
  src?: string;
  /**
   * The image bytes as standard base64, without a `data:` prefix: for images the client downloads
   * itself (the host does no HTTP). While non-empty it wins over `src`; `null` goes back to `src`.
   * Re-sending it makes the host read the bytes again.
   */
  data?: string;
  /** How the image fills the node's box. Default `contain`. */
  fit?: "contain" | "cover" | "fill" | "scale-down" | "none";
  class?: string;
}

export interface SwitchProps {
  checked?: boolean;
  label?: string;
  disabled?: boolean;
  /** The host only reports clicks; the client confirms them with `checked`. */
  controlled?: boolean;
  class?: string;
}

/** A choice of a `RadioGroup` or `Select`. */
export interface Option {
  value: string;
  label: string;
  disabled?: boolean;
}

export interface RadioGroupProps {
  options?: Option[];
  /** The selected option's `value`; one that is not an option selects nothing. */
  value?: string;
  direction?: "vertical" | "horizontal";
  disabled?: boolean;
  /** The host only reports choices; the client confirms them with `value`. */
  controlled?: boolean;
  class?: string;
}

export interface SelectProps {
  options?: Option[];
  /** The selected option's `value`; one that is not an option shows the placeholder. */
  value?: string;
  placeholder?: string;
  /** A search box at the top of the list. */
  searchable?: boolean;
  disabled?: boolean;
  /** The host only reports choices; the client confirms them with `value`. */
  controlled?: boolean;
  class?: string;
}

export interface SliderProps {
  value?: number;
  /** Default 0. */
  min?: number;
  /** Default 100. */
  max?: number;
  /** Above 0; default 1. */
  step?: number;
  disabled?: boolean;
  /** The host only reports drags; the client confirms them with `value`. */
  controlled?: boolean;
  class?: string;
}

export interface TextareaProps {
  value?: string;
  placeholder?: string;
  disabled?: boolean;
  class?: string;
}

/** A modal dialog; its children are its body (PROTOCOL.md section 14). */
export interface DialogProps {
  /** Shown while true; the host sets it to false when the user answers. */
  open?: boolean;
  title?: string;
  /** Default "OK". */
  okText?: string;
  /** Adds a cancel button. */
  cancelText?: string;
  /** In pixels. */
  width?: number;
  /** Classes of the body that holds the children. */
  class?: string;
}

/** A list that builds only the rows in view; each child is one row (PROTOCOL.md section 15). */
export interface VirtualListProps {
  /** Every row's height in pixels; default 32. */
  rowHeight?: number;
  /** Give the list a height (`flex-1`, `h-64`...): it fills its container. */
  class?: string;
}

/** A `Table` column; `key` names the cell in each row. */
export interface TableColumn {
  key: string;
  name: string;
  /** In pixels; default 100. */
  width?: number;
  align?: "left" | "center" | "right";
  /** Shows a sort button; the app sorts `rows` on `sort`. */
  sortable?: boolean;
}

export type TableCell = string | number | boolean | null;

export interface TableProps {
  columns?: TableColumn[];
  /** One object per row, with a cell per column `key`. */
  rows?: Record<string, TableCell>[];
  /** Index of the selected row in `rows`. */
  selected?: number;
  stripe?: boolean;
  /** Default true. */
  bordered?: boolean;
  /** The host only reports selections; the client confirms them with `selected`. */
  controlled?: boolean;
  class?: string;
}

/** A `sort` event: the column clicked and the direction to sort `rows` in. */
export interface TableSort {
  column: string;
  direction: "asc" | "desc" | "none";
}

/** A tab bar; the app shows the chosen tab's content itself. */
export interface TabsProps {
  tabs?: Option[];
  /** The selected tab's `value`. */
  value?: string;
  /** Default "tab". */
  variant?: "tab" | "outline" | "pill" | "segmented" | "underline";
  /** The host only reports choices; the client confirms them with `value`. */
  controlled?: boolean;
  class?: string;
}

/**
 * One tab of a `Tabs`: append children to build its chip (an `Icon`, a `text`, a `Tag`…; a
 * last `Button`, `Checkbox` or `Switch` is drawn after them, at its own size).
 * A `Tabs` with `Tab` children uses them and ignores its `tabs`, and the `Tab` reports nothing:
 * its `Tabs` sends the `change`.
 */
export interface TabProps {
  /** What `change` reports for this tab. Required: the host rejects a `Tab` without it. */
  value?: string;
  /** The tab's text, drawn before its children. A chip of nodes names itself with `a11y.label`. */
  label?: string;
  disabled?: boolean;
  /** Styles the chip itself: `flex-1` makes the tabs share the bar's width. */
  class?: string;
}

export interface MenuBarProps {
  menus?: Menu[];
  class?: string;
}

// Components from gpui-kit's Basic, Form, Layout and Advanced groups (PROTOCOL.md section 17).

export interface AlertProps {
  /** The text of the message. */
  message?: string;
  title?: string;
  /** Default "default". */
  variant?: "default" | "info" | "success" | "warning" | "error";
  /** A full-width band instead of a rounded box. */
  banner?: boolean;
  /** Shows a close button; a click sends `close`. */
  closable?: boolean;
  class?: string;
}

export interface BadgeProps {
  /** Default 0; a count of 0 hides the badge. */
  count?: number;
  /** Default 99: a bigger count shows `max+`. */
  max?: number;
  /** A dot instead of a count. */
  dot?: boolean;
  /** A bundled icon instead of a count. */
  icon?: string;
  class?: string;
}

export interface TagProps {
  label?: string;
  /** Default "secondary". */
  variant?: "primary" | "secondary" | "danger" | "success" | "warning" | "info";
  outline?: boolean;
  icon?: string;
  class?: string;
}

export interface KbdProps {
  /** A key such as `ctrl-s`. */
  keys?: string;
  outline?: boolean;
  class?: string;
}

export interface ProgressProps {
  /** From 0 to 100. */
  value?: number;
  loading?: boolean;
  class?: string;
}

export type SpinnerProps = { class?: string };

export interface SkeletonProps {
  secondary?: boolean;
  class?: string;
}

export interface RatingProps {
  /** Default 0. */
  value?: number;
  /** Default 5. */
  max?: number;
  disabled?: boolean;
  /** The host only reports choices; the client confirms them with `value`. */
  controlled?: boolean;
  class?: string;
}

export interface PaginationProps {
  /** 1-based; default 1. */
  page?: number;
  /** Default 1. */
  total?: number;
  compact?: boolean;
  disabled?: boolean;
  /** The host only reports choices; the client confirms them with `page`. */
  controlled?: boolean;
  class?: string;
}

/** A section of an `Accordion`; its content is text. */
export interface AccordionItem {
  id: string;
  title: string;
  content?: string;
  open?: boolean;
  disabled?: boolean;
}

export interface AccordionProps {
  items?: AccordionItem[];
  /** Several sections open at once; default false. */
  multiple?: boolean;
  /** Default true. */
  bordered?: boolean;
  disabled?: boolean;
  /** The host only reports toggles; the client confirms them with `items`. */
  controlled?: boolean;
  class?: string;
}

export interface CollapsibleProps {
  open?: boolean;
  class?: string;
}

export interface AvatarProps {
  /** Shown as its initial when there is no `src`. */
  name?: string;
  /** An image file. */
  src?: string;
  class?: string;
}

export interface StepperProps {
  /** The steps, as `{value, label}`; `disabled` skips one. */
  items?: Option[];
  /** The current step's `value`. */
  value?: string;
  vertical?: boolean;
  disabled?: boolean;
  /** The host only reports choices; the client confirms them with `value`. */
  controlled?: boolean;
  class?: string;
}

export interface NumberInputProps {
  value?: string;
  placeholder?: string;
  disabled?: boolean;
  /** The host only reports edits; the client confirms them with `value`. */
  controlled?: boolean;
  class?: string;
}

export interface OtpInputProps {
  /** The number of cells; default 6. */
  length?: number;
  value?: string;
  masked?: boolean;
  disabled?: boolean;
  /** The host only reports edits; the client confirms them with `value`. */
  controlled?: boolean;
  class?: string;
}

export interface FormProps {
  /** Default "vertical". */
  direction?: "vertical" | "horizontal";
  class?: string;
}

export interface FieldProps {
  label?: string;
  description?: string;
  required?: boolean;
  class?: string;
}

export interface GroupBoxProps {
  title?: string;
  /** Default "normal". */
  variant?: "normal" | "fill" | "outline";
  class?: string;
}

/** One row of a `DescriptionList`. */
export interface DescriptionItem {
  label: string;
  value: string;
  /** How many columns the row spans; default 1. */
  span?: number;
}

export interface DescriptionListProps {
  items?: DescriptionItem[];
  /** Default 1. */
  columns?: number;
  bordered?: boolean;
  /** Label and value side by side; default stacked. */
  horizontal?: boolean;
  class?: string;
}

export interface StatusBarProps {
  left?: string;
  right?: string;
  class?: string;
}

export type ToolbarProps = { class?: string };

/** Panels side by side (default) or stacked; one child per panel. */
export interface ResizableProps {
  axis?: "horizontal" | "vertical";
  /** The initial size in pixels of each panel, in order; the others share what is left. */
  sizes?: number[];
  class?: string;
}

export interface CalendarProps {
  /** The chosen day, `YYYY-MM-DD`. */
  value?: string;
  /** How many months to show, 1 to 3; default 1. */
  months?: number;
  /** Default "sunday". */
  weekStartsOn?: "sunday" | "monday";
  disabled?: boolean;
  /** The host only reports choices; the client confirms them with `value`. */
  controlled?: boolean;
  class?: string;
}

/** One item of a `Tree`; `children` nests them. */
export interface TreeNode {
  /** Unique within the whole `items` list. */
  id: string;
  label: string;
  children?: TreeNode[];
  /** Open when the tree first shows it; the user can expand or collapse it. */
  expanded?: boolean;
  disabled?: boolean;
}

export interface TreeProps {
  items?: TreeNode[];
  /** The selected item's `id`; one that is not in `items` selects nothing. */
  value?: string;
  /** The host only reports clicks; the client confirms them with `value`. */
  controlled?: boolean;
  /** Give the tree a bounded height (`h-64`, `flex-1`): it scrolls itself. */
  class?: string;
}

/** The selected source lines of a `Diff`, reported with `change`. */
export interface DiffSelection {
  path: string;
  side: "original" | "modified";
  start: number;
  end: number;
  endSide: "original" | "modified";
}

export interface DiffProps {
  /** A unified or Git diff; a malformed patch is `invalid_props`. */
  patch?: string;
  /** Default "unified". */
  mode?: "unified" | "split";
  /** Default true. */
  lineNumbers?: boolean;
  /** Default false: long lines scroll horizontally. */
  softWrap?: boolean;
  /** Shows a header above each file; default true. */
  header?: boolean;
  /** Give it a bounded height (`h-96`, `flex-1`): it scrolls itself. */
  class?: string;
}

/** A carousel; each child is one slide. */
export interface CarouselProps {
  /** The selected slide index; one past the last slide selects the last one. */
  value?: number;
  /** Default "horizontal". */
  axis?: "horizontal" | "vertical";
  /** Wrap from the last slide to the first; default false. */
  looping?: boolean;
  /** Previous and next buttons; default true. */
  controls?: boolean;
  /** One dot per slide; default false. */
  pagination?: boolean;
  /** The host only reports slides; the client confirms them with `value`. */
  controlled?: boolean;
  class?: string;
}

/** One entry of a `Sidebar`; `children` is its submenu. */
export interface SidebarEntry {
  /** Unique across the whole sidebar: it is the `value` and what `change` reports. */
  id: string;
  label: string;
  /** A bundled icon name; collapsed, it is all the entry shows. */
  icon?: string;
  disabled?: boolean;
  /** The submenu starts open; default false. */
  open?: boolean;
  children?: SidebarEntry[];
}

/** A group of entries with a heading (`label`); without one the entries have no heading. */
export interface SidebarSection {
  label?: string;
  items: SidebarEntry[];
}

export interface SidebarProps {
  /** The sections: the sidebar takes no children. */
  items?: SidebarSection[];
  /** The active entry's `id`; one that is not in `items` leaves none active. */
  value?: string;
  /** Icon width (48px) instead of the full width. */
  collapsed?: boolean;
  /** What collapsing does: `icon` (default), `offcanvas` or `none`. */
  collapsible?: "icon" | "offcanvas" | "none";
  /** Default `left`. */
  side?: "left" | "right";
  /** The host only reports clicks; the client confirms them with `value`. */
  controlled?: boolean;
  class?: string;
}

/** A theme color of a chart series (PROTOCOL.md section 19). */
export type ChartColor =
  | "chart-1"
  | "chart-2"
  | "chart-3"
  | "chart-4"
  | "chart-5"
  | "primary"
  | "secondary"
  | "muted"
  | "accent"
  | "danger"
  | "success"
  | "warning"
  | "info";

/** One point of a `Chart`: the category of a line or a bar, or the name of a slice. */
export interface ChartDatum {
  label: string;
  value: number;
}

export interface ChartProps {
  /** The plot: `line` (default), `bar` or `pie`. */
  chart?: "line" | "bar" | "pie";
  /** The points, in order. */
  data?: ChartDatum[];
  /** A line's or the bars' color; default `chart-1`. A pie cycles `chart-1` to `chart-5`. */
  color?: ChartColor;
  /** The series name the hover tooltip shows. */
  name?: string;
  /** A line's curve between points; default `linear`. */
  curve?: "linear" | "natural" | "step";
  /** A line: a dot on each point; default false. */
  dot?: boolean;
  /** Bars grow from the left instead of the bottom; default false. */
  horizontal?: boolean;
  /** The category axis (of a line or the bars); default true. */
  xAxis?: boolean;
  /** How many category labels, spread evenly; default 12, 0 none. */
  xTicks?: number;
  /** The value axis; default false. */
  yAxis?: boolean;
  /** Ticks on the value axis, both ends included; default 5, below 2 counts as 2. */
  yTicks?: number;
  /** The grid lines; default true. */
  grid?: boolean;
  /** A line: pin the value axis; both are needed, in order. */
  yMin?: number;
  yMax?: number;
  /** A pie: the hole in the middle, in pixels; default 0. */
  innerRadius?: number;
  /** A pie: the gap between slices, in radians (0.03 is a thin gap); default 0. */
  padAngle?: number;
  /** The hover crosshair, the dots and the tooltip; default true. */
  interactive?: boolean;
  /** Give it a bounded box (`h-64`, `flex-1 w-full`): the plot fills it. */
  class?: string;
}

export interface NodeProps {
  div: DivProps & SharedProps;
  text: TextProps & SharedProps;
  Button: ButtonProps & SharedProps;
  Input: InputProps & SharedProps;
  Checkbox: CheckboxProps & SharedProps;
  Icon: IconProps & SharedProps;
  Image: ImageProps & SharedProps;
  MenuBar: MenuBarProps & SharedProps;
  Switch: SwitchProps & SharedProps;
  RadioGroup: RadioGroupProps & SharedProps;
  Select: SelectProps & SharedProps;
  Slider: SliderProps & SharedProps;
  Textarea: TextareaProps & SharedProps;
  Dialog: DialogProps & SharedProps;
  VirtualList: VirtualListProps & SharedProps;
  Table: TableProps & SharedProps;
  Tabs: TabsProps & SharedProps;
  Tab: TabProps & SharedProps;
  Alert: AlertProps & SharedProps;
  Badge: BadgeProps & SharedProps;
  Tag: TagProps & SharedProps;
  Kbd: KbdProps & SharedProps;
  Progress: ProgressProps & SharedProps;
  Spinner: SpinnerProps & SharedProps;
  Skeleton: SkeletonProps & SharedProps;
  Rating: RatingProps & SharedProps;
  Pagination: PaginationProps & SharedProps;
  Collapsible: CollapsibleProps & SharedProps;
  Accordion: AccordionProps & SharedProps;
  Avatar: AvatarProps & SharedProps;
  Stepper: StepperProps & SharedProps;
  NumberInput: NumberInputProps & SharedProps;
  OtpInput: OtpInputProps & SharedProps;
  Form: FormProps & SharedProps;
  Field: FieldProps & SharedProps;
  GroupBox: GroupBoxProps & SharedProps;
  DescriptionList: DescriptionListProps & SharedProps;
  StatusBar: StatusBarProps & SharedProps;
  Toolbar: ToolbarProps & SharedProps;
  Resizable: ResizableProps & SharedProps;
  Calendar: CalendarProps & SharedProps;
  Tree: TreeProps & SharedProps;
  Diff: DiffProps & SharedProps;
  Carousel: CarouselProps & SharedProps;
  Sidebar: SidebarProps & SharedProps;
  Chart: ChartProps & SharedProps;
}

// Event payloads per node type (PROTOCOL.md section 6).

export interface NodeEvents {
  div: { click: {}; doubleClick: {}; menu: { item: string }; keyDown: { key: string }; drop: { paths: string[] } };
  text: {};
  Button: { click: {} };
  Input: { change: { value: string }; submit: { value: string }; keyDown: { key: string } };
  Checkbox: { change: { checked: boolean } };
  Icon: {};
  Image: {};
  MenuBar: { menu: { item: string } };
  Switch: { change: { checked: boolean } };
  RadioGroup: { change: { value: string } };
  Select: { change: { value: string } };
  Slider: { change: { value: number } };
  Textarea: { change: { value: string } };
  Dialog: { ok: {}; cancel: {} };
  VirtualList: {};
  /** `value` is a row index; `null` when Escape clears the selection. */
  Table: { change: { value: number | null }; doubleClick: { value: number }; sort: TableSort };
  Tabs: { change: { value: string } };
  /** A `Tab` reports nothing: its `Tabs` sends the `change`. */
  Tab: {};
  Alert: { close: {} };
  Badge: {};
  Tag: {};
  Kbd: {};
  Progress: {};
  Spinner: {};
  Skeleton: {};
  Rating: { change: { value: number } };
  Pagination: { change: { value: number } };
  Collapsible: {};
  /** `value` is the `id` of the section opened or closed. */
  Accordion: { change: { value: string } };
  Avatar: {};
  /** `value` is the chosen step's `value`. */
  Stepper: { change: { value: string } };
  NumberInput: { change: { value: string }; submit: { value: string }; keyDown: { key: string } };
  OtpInput: { change: { value: string } };
  Form: {};
  Field: {};
  GroupBox: {};
  DescriptionList: {};
  StatusBar: {};
  Toolbar: {};
  Resizable: {};
  /** `value` is the chosen day, `YYYY-MM-DD`. */
  Calendar: { change: { value: string } };
  /** `value` is the selected item's `id`; `expand`/`collapse` report a folder's `id`. */
  Tree: { change: { value: string }; expand: { value: string }; collapse: { value: string } };
  /** `value` is the selected range, `null` when the selection is cleared. */
  Diff: { change: { value: DiffSelection | null } };
  /** `value` is the selected slide index. */
  Carousel: { change: { value: number } };
  /** `value` is the `id` of the entry clicked. */
  Sidebar: { change: { value: string } };
  /** A chart has no events: the hover tooltip is the component's own. */
  Chart: {};
}
