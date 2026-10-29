// Host components. Each is the node type string at runtime (React treats string
// element types as host components), typed as a component for JSX.

import type { ReactNode, Ref } from "react";
import type {
  A11y,
  ButtonProps as WireButtonProps,
  DiffSelection,
  ImageProps as WireImageProps,
  Menu,
  MenuItem,
  NodeProps,
  NodeType,
  Option,
  QuestionnaireAnswer,
  RadioGroupProps as WireRadioGroupProps,
  SettingsChange,
  TableCell,
  TableColumn,
  TableSort,
  TabsProps as WireTabsProps,
  UINode,
  WindowControl,
  WindowEventPayload,
} from "gpui-bridge";

/** A `ref` gets the node (`ref.current.focus()`). */
type HostComponent<P> = (props: P & { ref?: Ref<UINode<NodeType>> }) => ReactNode;

/** Props every node accepts (PROTOCOL.md section 16). */
export interface SharedProps {
  /** Accessibility: a node without it is not reported to assistive technology. */
  a11y?: A11y;
}

export interface DivProps extends SharedProps {
  class?: string;
  /** Makes the div a title bar part handled by the system: drag area or minimize/maximize/close button. */
  windowControl?: WindowControl;
  /** Opens on right-click; choosing an item calls `onMenu` with its `id`. */
  contextMenu?: MenuItem[];
  onClick?: () => void;
  onDoubleClick?: () => void;
  onMenu?: (item: string) => void;
  /** Shown when the pointer rests on the div. */
  tooltip?: string;
  /** Files dropped on the div. */
  onDrop?: (paths: string[]) => void;
  /** Makes the div focusable; called with keys such as `escape` or `ctrl-enter`. */
  onKeyDown?: (key: string) => void;
  children?: ReactNode;
}

export interface TextProps extends SharedProps {
  class?: string;
  /** Text content: strings and numbers. */
  children?: ReactNode;
}

export interface ButtonProps extends SharedProps {
  /** Alternatively, pass the label as text children. */
  label?: string;
  variant?: WireButtonProps["variant"];
  disabled?: boolean;
  tooltip?: string;
  class?: string;
  onClick?: () => void;
  children?: ReactNode;
}

export interface InputProps extends SharedProps {
  /** Passing `value` makes the input controlled (the host waits for React to confirm edits). */
  value?: string;
  placeholder?: string;
  disabled?: boolean;
  class?: string;
  onChange?: (value: string) => void;
  onSubmit?: (value: string) => void;
  onKeyDown?: (key: string) => void;
}

export interface CheckboxProps extends SharedProps {
  /** Passing `checked` makes the checkbox controlled (the host waits for React to confirm clicks). */
  checked?: boolean;
  label?: string;
  disabled?: boolean;
  class?: string;
  onChange?: (checked: boolean) => void;
}

export interface IconProps extends SharedProps {
  /** A bundled icon, by its Lucide name (`folder`, `file-text`…). */
  name: string;
  class?: string;
}

export interface ImageProps extends SharedProps {
  /** A local image file; a relative path is resolved against the host's working directory. */
  src?: string;
  /** The image bytes as standard base64 (no `data:` prefix), for images the client downloads
   * itself. While non-empty it wins over `src`. */
  data?: string;
  fit?: WireImageProps["fit"];
  class?: string;
}

export interface SwitchProps extends SharedProps {
  /** Passing `checked` makes the switch controlled (the host waits for React to confirm clicks). */
  checked?: boolean;
  label?: string;
  disabled?: boolean;
  class?: string;
  onChange?: (checked: boolean) => void;
}

export interface RadioGroupProps extends SharedProps {
  options: Option[];
  /** Passing `value` makes the group controlled (the host waits for React to confirm choices). */
  value?: string;
  direction?: WireRadioGroupProps["direction"];
  disabled?: boolean;
  class?: string;
  onChange?: (value: string) => void;
}

export interface SelectProps extends SharedProps {
  options: Option[];
  /** Passing `value` makes the select controlled (the host waits for React to confirm choices). */
  value?: string;
  placeholder?: string;
  searchable?: boolean;
  disabled?: boolean;
  class?: string;
  onChange?: (value: string) => void;
}

export interface SliderProps extends SharedProps {
  /** Passing `value` makes the slider controlled (the host waits for React to confirm drags). */
  value?: number;
  min?: number;
  max?: number;
  step?: number;
  disabled?: boolean;
  class?: string;
  onChange?: (value: number) => void;
}

export interface TextareaProps extends SharedProps {
  /** The host keeps the text; a `value` that differs from it replaces it. */
  value?: string;
  placeholder?: string;
  disabled?: boolean;
  class?: string;
  onChange?: (value: string) => void;
}

export interface MenuBarProps extends SharedProps {
  menus: Menu[];
  class?: string;
  /** Called with the chosen item's `id`. */
  onMenu?: (item: string) => void;
}

export interface TrayProps {
  /** An image file (PNG, ICO…). */
  icon?: string;
  tooltip?: string;
  /** Opens on right-click; a native menu, so item icons are not drawn. */
  menu?: MenuItem[];
  onClick?: () => void;
  /** Called with the chosen item's `id`. */
  onMenu?: (item: string) => void;
}

export interface DialogProps extends SharedProps {
  /** Shown while true. The host closes it when the user answers: set your state in onOk/onCancel. */
  open: boolean;
  title?: string;
  okText?: string;
  /** Adds a cancel button. */
  cancelText?: string;
  width?: number;
  /** Classes of the body that holds the children. */
  class?: string;
  onOk?: () => void;
  /** Cancel button, Escape, the X or a click outside. */
  onCancel?: () => void;
  children?: ReactNode;
}

export interface VirtualListProps extends SharedProps {
  /** Every row's height in pixels; default 32. */
  rowHeight?: number;
  /** Give the list a height (`flex-1`, `h-64`...): it fills its container. */
  class?: string;
  /** One child per row; only the rows in view are drawn. */
  children?: ReactNode;
}

export interface TableProps extends SharedProps {
  columns: TableColumn[];
  /** One object per row. Keep it in state: a new array sends every row again. */
  rows: Record<string, TableCell>[];
  /** Passing `selected` makes the selection controlled (the host waits for React to confirm it). */
  selected?: number | null;
  stripe?: boolean;
  bordered?: boolean;
  class?: string;
  /** A row index; `null` when Escape clears the selection. */
  onChange?: (row: number | null) => void;
  onDoubleClick?: (row: number) => void;
  /** Sort `rows` yourself and render them again. */
  onSort?: (sort: TableSort) => void;
}

export interface TabsProps extends SharedProps {
  /** Ignored when the `Tabs` has `Tab` children. */
  tabs?: Option[];
  /** Passing `value` makes the tabs controlled (the host waits for React to confirm choices). */
  value?: string;
  variant?: WireTabsProps["variant"];
  class?: string;
  onChange?: (value: string) => void;
  /** One `Tab` per child. */
  children?: ReactNode;
}

export interface TabProps extends SharedProps {
  /** What `change` reports for this tab. Required: the host rejects a `Tab` without it. */
  value?: string;
  /** The tab's text, drawn before its children. A chip of nodes names itself with `a11y.label`. */
  label?: string;
  disabled?: boolean;
  /** Nodes inside the chip, e.g. an `Icon`, a `Text` and a `Tag`; a last `Button` is drawn after them. */
  children?: ReactNode;
  /** Styles the chip itself: `flex-1` makes the tabs share the bar's width. */
  class?: string;
}

export interface WindowProps {
  title?: string;
  width?: number;
  height?: number;
  /** Initial position; only read when the window opens. */
  x?: number;
  y?: number;
  /** Minimum size the user can resize to; only read when the window opens. */
  minWidth?: number;
  minHeight?: number;
  /** Minimizes (`true`) or restores (`false`) on the next commit. */
  minimized?: boolean;
  /** No system title bar; only read when the window opens. */
  frameless?: boolean;
  /** Classes of the window's root div (default `size-full`). */
  class?: string;
  /** The user closed the window. Unmount the `Window` (or React keeps an element for a window that is gone). */
  onClose?: () => void;
  /** The window gained the system focus. */
  onFocus?: (event: WindowEventPayload) => void;
  /** The window lost the system focus. */
  onBlur?: (event: WindowEventPayload) => void;
  /** The user resized the window: the payload has `x`, `y`, `width`, `height`. */
  onResize?: (bounds: WindowEventPayload) => void;
  /** The user moved the window: the payload has `x`, `y`, `width`, `height`. */
  onMove?: (bounds: WindowEventPayload) => void;
  /** The window is being presented or not (`visible`); minimizing reports it too. */
  onVisibility?: (event: WindowEventPayload) => void;
  children?: ReactNode;
}

export const Div = "div" as unknown as HostComponent<DivProps>;
export const Text = "text" as unknown as HostComponent<TextProps>;
export const Button = "Button" as unknown as HostComponent<ButtonProps>;
export const Input = "Input" as unknown as HostComponent<InputProps>;
export const Checkbox = "Checkbox" as unknown as HostComponent<CheckboxProps>;
export const Icon = "Icon" as unknown as HostComponent<IconProps>;
export const Image = "Image" as unknown as HostComponent<ImageProps>;
export const MenuBar = "MenuBar" as unknown as HostComponent<MenuBarProps>;
export const Switch = "Switch" as unknown as HostComponent<SwitchProps>;
export const RadioGroup = "RadioGroup" as unknown as HostComponent<RadioGroupProps>;
export const Select = "Select" as unknown as HostComponent<SelectProps>;
export const Slider = "Slider" as unknown as HostComponent<SliderProps>;
export const Textarea = "Textarea" as unknown as HostComponent<TextareaProps>;
/** A modal dialog; its children are its body. */
export const Dialog = "Dialog" as unknown as HostComponent<DialogProps>;
/** A list that draws only the rows in view; each child is a row of `rowHeight`. */
export const VirtualList = "VirtualList" as unknown as HostComponent<VirtualListProps>;
/** A data table with text cells; it draws only the rows in view. */
export const Table = "Table" as unknown as HostComponent<TableProps>;
/** A tab bar; render the chosen tab's content yourself. */
export const Tabs = "Tabs" as unknown as HostComponent<TabsProps>;
/** One tab of a `Tabs`: its children are the nodes of its chip. */
export const Tab = "Tab" as unknown as HostComponent<TabProps>;
/**
 * Shows the system tray icon while mounted (one per app). While it is shown, closing the
 * last window does not end the app; unmounting it then does.
 */
export const Tray = "Tray" as unknown as HostComponent<TrayProps>;
/** Opens another window while mounted; its children render there. Closes when unmounted. */
export const Window = "Window" as unknown as HostComponent<WindowProps>;

// gpui-kit's Basic, Form, Layout and Advanced groups (PROTOCOL.md section 17). Their props are
// the wire props, plus the React handlers and children.

export type AlertProps = NodeProps["Alert"] & { onClose?: () => void; children?: ReactNode };
export const Alert = "Alert" as unknown as HostComponent<AlertProps>;

export type BadgeProps = NodeProps["Badge"] & { children?: ReactNode };
export const Badge = "Badge" as unknown as HostComponent<BadgeProps>;

export type TagProps = NodeProps["Tag"];
export const Tag = "Tag" as unknown as HostComponent<TagProps>;

export type KbdProps = NodeProps["Kbd"];
export const Kbd = "Kbd" as unknown as HostComponent<KbdProps>;

export type ProgressProps = NodeProps["Progress"];
export const Progress = "Progress" as unknown as HostComponent<ProgressProps>;

export type SpinnerProps = NodeProps["Spinner"];
export const Spinner = "Spinner" as unknown as HostComponent<SpinnerProps>;

export type SkeletonProps = NodeProps["Skeleton"];
export const Skeleton = "Skeleton" as unknown as HostComponent<SkeletonProps>;

export type RatingProps = NodeProps["Rating"] & { onChange?: (value: number) => void };
export const Rating = "Rating" as unknown as HostComponent<RatingProps>;

export type PaginationProps = NodeProps["Pagination"] & { onChange?: (page: number) => void };
export const Pagination = "Pagination" as unknown as HostComponent<PaginationProps>;

export type CollapsibleProps = NodeProps["Collapsible"] & { children?: ReactNode };
export const Collapsible = "Collapsible" as unknown as HostComponent<CollapsibleProps>;

export type AccordionProps = NodeProps["Accordion"] & { onChange?: (id: string) => void };
export const Accordion = "Accordion" as unknown as HostComponent<AccordionProps>;

export type AvatarProps = NodeProps["Avatar"];
export const Avatar = "Avatar" as unknown as HostComponent<AvatarProps>;

export type StepperProps = NodeProps["Stepper"] & { onChange?: (value: string) => void };
export const Stepper = "Stepper" as unknown as HostComponent<StepperProps>;

export type NumberInputProps = NodeProps["NumberInput"] & { onChange?: (value: string) => void; onSubmit?: (value: string) => void };
export const NumberInput = "NumberInput" as unknown as HostComponent<NumberInputProps>;

export type OtpInputProps = NodeProps["OtpInput"] & { onChange?: (value: string) => void };
export const OtpInput = "OtpInput" as unknown as HostComponent<OtpInputProps>;

export type FormProps = NodeProps["Form"] & { children?: ReactNode };
export const Form = "Form" as unknown as HostComponent<FormProps>;

export type FieldProps = NodeProps["Field"] & { children?: ReactNode };
export const Field = "Field" as unknown as HostComponent<FieldProps>;

export type GroupBoxProps = NodeProps["GroupBox"] & { children?: ReactNode };
export const GroupBox = "GroupBox" as unknown as HostComponent<GroupBoxProps>;

export type DescriptionListProps = NodeProps["DescriptionList"];
export const DescriptionList = "DescriptionList" as unknown as HostComponent<DescriptionListProps>;

export type StatusBarProps = NodeProps["StatusBar"];
export const StatusBar = "StatusBar" as unknown as HostComponent<StatusBarProps>;

export type ToolbarProps = NodeProps["Toolbar"] & { children?: ReactNode };
export const Toolbar = "Toolbar" as unknown as HostComponent<ToolbarProps>;

export type ResizableProps = NodeProps["Resizable"] & { children?: ReactNode };
export const Resizable = "Resizable" as unknown as HostComponent<ResizableProps>;

export type CalendarProps = NodeProps["Calendar"] & { onChange?: (day: string) => void };
export const Calendar = "Calendar" as unknown as HostComponent<CalendarProps>;

export type TreeProps = NodeProps["Tree"] & {
  onChange?: (id: string) => void;
  onExpand?: (id: string) => void;
  onCollapse?: (id: string) => void;
};
export const Tree = "Tree" as unknown as HostComponent<TreeProps>;

/** The selected lines, or `null` when the selection is cleared. */
export type DiffProps = NodeProps["Diff"] & { onChange?: (selection: DiffSelection | null) => void };
export const Diff = "Diff" as unknown as HostComponent<DiffProps>;

/** Each child is one slide. */
export type CarouselProps = NodeProps["Carousel"] & {
  onChange?: (index: number) => void;
  children?: ReactNode;
};
export const Carousel = "Carousel" as unknown as HostComponent<CarouselProps>;

/** The sections are data; `onChange` gets the clicked entry's id. */
export type SidebarProps = NodeProps["Sidebar"] & { onChange?: (id: string) => void };
export const Sidebar = "Sidebar" as unknown as HostComponent<SidebarProps>;

/** A line, bar or pie chart of the points in `data`; it has no events of its own. */
export type ChartProps = NodeProps["Chart"];
export const Chart = "Chart" as unknown as HostComponent<ChartProps>;

/** Paints the drawing commands in `commands`; `onPointerDown` gets the local point. */
export type CanvasProps = NodeProps["Canvas"] & { onPointerDown?: (point: { x: number; y: number }) => void };
export const Canvas = "Canvas" as unknown as HostComponent<CanvasProps>;

/** A command palette over the sections in `items`; `onConfirm` gets the chosen entry's `id`. */
export type CommandProps = NodeProps["Command"] & { onConfirm?: (id: string) => void };
export const Command = "Command" as unknown as HostComponent<CommandProps>;

/** Settings pages of typed fields; `onChange` gets `{key, value}` of the field the user set. */
export type SettingsProps = NodeProps["Settings"] & {
  onChange?: (change: SettingsChange) => void;
};
export const Settings = "Settings" as unknown as HostComponent<SettingsProps>;

/** A button that stays pressed; `onChange` gets the new `checked`. */
export type ToggleProps = NodeProps["Toggle"] & { onChange?: (checked: boolean) => void };
export const Toggle = "Toggle" as unknown as HostComponent<ToggleProps>;

/** An action with a menu: `onClick` is the left half, `onMenu` gets the chosen item's id. */
export type DropdownButtonProps = NodeProps["DropdownButton"] & {
  onClick?: () => void;
  onMenu?: (item: string) => void;
};
export const DropdownButton = "DropdownButton" as unknown as HostComponent<DropdownButtonProps>;

export type LabelProps = NodeProps["Label"];
export const Label = "Label" as unknown as HostComponent<LabelProps>;

/** A centered empty state; its children are the action under the header. */
export type EmptyProps = NodeProps["Empty"] & { children?: ReactNode };
export const Empty = "Empty" as unknown as HostComponent<EmptyProps>;

export type ShimmerProps = NodeProps["Shimmer"];
export const Shimmer = "Shimmer" as unknown as HostComponent<ShimmerProps>;

/** A framed text field; `onChange` and `onSubmit` work as in an `Input`. */
export type InputGroupProps = NodeProps["InputGroup"] & {
  onChange?: (value: string) => void;
  onSubmit?: (value: string) => void;
};
export const InputGroup = "InputGroup" as unknown as HostComponent<InputGroupProps>;

/** Copies `value` on click; `onCopied` gets the text that was copied. */
export type ClipboardProps = NodeProps["Clipboard"] & { onCopied?: (value: string) => void };
export const Clipboard = "Clipboard" as unknown as HostComponent<ClipboardProps>;

/** A confirmation; `onOk` and `onCancel` close it, as in a `Dialog`. */
export type AlertDialogProps = NodeProps["AlertDialog"] & {
  onOk?: () => void;
  onCancel?: () => void;
  children?: ReactNode;
};
export const AlertDialog = "AlertDialog" as unknown as HostComponent<AlertDialogProps>;

/** A panel that slides in from `side`; `onClose` is its X, Escape or a click on the overlay. */
export type SheetProps = NodeProps["Sheet"] & { onClose?: () => void; children?: ReactNode };
export const Sheet = "Sheet" as unknown as HostComponent<SheetProps>;

/**
 * A panel anchored to its first child (the trigger); the rest are the panel. `onChange` gets the
 * new `open`: the host keeps it, as it keeps a `Checkbox`'s `checked`.
 */
export type PopoverProps = NodeProps["Popover"] & { onChange?: (checked: boolean) => void; children?: ReactNode };
export const Popover = "Popover" as unknown as HostComponent<PopoverProps>;

/** A card on hover, opened by the mouse alone; `onChange` gets the new `open`. */
export type HoverCardProps = NodeProps["HoverCard"] & { onChange?: (checked: boolean) => void; children?: ReactNode };
export const HoverCard = "HoverCard" as unknown as HostComponent<HoverCardProps>;

/** A day picker; `onChange` gets the chosen day, `null` when cleared. */
export type DatePickerProps = NodeProps["DatePicker"] & { onChange?: (value: string | null) => void };
export const DatePicker = "DatePicker" as unknown as HostComponent<DatePickerProps>;

/** A time typed by segments; `onChange` gets the time (`HH:MM[:SS]`). */
export type TimeFieldProps = NodeProps["TimeField"] & { onChange?: (value: string) => void };
export const TimeField = "TimeField" as unknown as HostComponent<TimeFieldProps>;

/** A color swatch and hex box; `onChange` gets the color, `null` when cleared. */
export type ColorPickerProps = NodeProps["ColorPicker"] & { onChange?: (value: string | null) => void };
export const ColorPicker = "ColorPicker" as unknown as HostComponent<ColorPickerProps>;

/** A list that picks several options; `onChange` gets the whole selection. */
export type ComboboxProps = NodeProps["Combobox"] & { onChange?: (value: string[]) => void };
export const Combobox = "Combobox" as unknown as HostComponent<ComboboxProps>;

// Chat (PROTOCOL.md section 26).

/** A chat bubble; its children are the content. */
export type BubbleProps = NodeProps["Bubble"] & { children?: ReactNode };
export const Bubble = "Bubble" as unknown as HostComponent<BubbleProps>;

/** A message row: a `MessageHeader` and a `MessageFooter` child, the rest is the content. */
export type MessageProps = NodeProps["Message"] & { children?: ReactNode };
export const Message = "Message" as unknown as HostComponent<MessageProps>;

export type MessageHeaderProps = NodeProps["MessageHeader"] & { children?: ReactNode };
export const MessageHeader = "MessageHeader" as unknown as HostComponent<MessageHeaderProps>;

export type MessageFooterProps = NodeProps["MessageFooter"] & { children?: ReactNode };
export const MessageFooter = "MessageFooter" as unknown as HostComponent<MessageFooterProps>;

/** A compact status row; children go after the text. */
export type MarkerProps = NodeProps["Marker"] & { children?: ReactNode };
export const Marker = "Marker" as unknown as HostComponent<MarkerProps>;

/** A file or picture card; each handler shows its control (`onRetry` only while `failed`). */
export type AttachmentProps = NodeProps["Attachment"] & {
  onClick?: () => void;
  onRemove?: () => void;
  onRetry?: () => void;
  children?: ReactNode;
};
export const Attachment = "Attachment" as unknown as HostComponent<AttachmentProps>;

/** A virtual list of messages anchored at the end; each child is a row. */
export type MessageScrollerProps = NodeProps["MessageScroller"] & { children?: ReactNode };
export const MessageScroller = "MessageScroller" as unknown as HostComponent<MessageScrollerProps>;

/** Questions one at a time; `onChange` gets one answer, `onSubmit` all of them. */
export type QuestionnaireProps = NodeProps["Questionnaire"] & {
  onChange?: (answer: QuestionnaireAnswer) => void;
  onSubmit?: (answers: QuestionnaireAnswer[]) => void;
};
export const Questionnaire = "Questionnaire" as unknown as HostComponent<QuestionnaireProps>;
