package gpuibridge

// Window is a window opened by the client.
type Window struct {
	app *App
	id  WindowID
}

// ID returns the window's id.
func (w *Window) ID() WindowID { return w.id }

// Close closes the window on the next commit (PROTOCOL.md §3).
func (w *Window) Close() {
	w.app.mu.Lock()
	defer w.app.mu.Unlock()
	w.app.tree.pending = append(w.app.tree.pending, closeWindowMsg(w.id))
}

// Window creates (or updates) the main window (id 1) and attaches it to the tree.
func (a *App) Window(opts WindowOptions) {
	a.queueWindow(opts, 1)
}

// CreateWindow opens an extra window with a client-assigned id.
func (a *App) CreateWindow(opts WindowOptions) *Window {
	a.mu.Lock()
	id := a.nextWindowID
	a.nextWindowID++
	w := &Window{app: a, id: id}
	a.mu.Unlock()
	a.queueWindow(opts, id)
	return w
}

func (a *App) queueWindow(opts WindowOptions, id WindowID) {
	var rootId *NodeID
	if opts.Root != nil {
		rootId = &opts.Root.id
	}
	a.mu.Lock()
	a.tree.pending = append(a.tree.pending, windowMsg(id, opts.Title, opts.Width, opts.Height,
		opts.X, opts.Y, opts.MinWidth, opts.MinHeight, rootId, opts.Theme, opts.ThemeSet,
		opts.Frameless, opts.Minimized, opts.Events))
	a.mu.Unlock()
}

// --- constructors per node type (PROTOCOL.md §5) ---------------------------

func (a *App) Create(kind string, props Props) *Node {
	a.mu.Lock()
	defer a.mu.Unlock()
	return &Node{app: a, id: a.tree.create(kind, props, nil), typ: kind}
}

func (a *App) Div(props Props) *Node             { return a.Create("div", props) }
func (a *App) Text(props Props) *Node            { return a.Create("text", props) }
func (a *App) Button(props Props) *Node          { return a.Create("Button", props) }
func (a *App) Input(props Props) *Node           { return a.Create("Input", props) }
func (a *App) Checkbox(props Props) *Node        { return a.Create("Checkbox", props) }
func (a *App) Switch(props Props) *Node          { return a.Create("Switch", props) }
func (a *App) RadioGroup(props Props) *Node      { return a.Create("RadioGroup", props) }
func (a *App) Select(props Props) *Node          { return a.Create("Select", props) }
func (a *App) Slider(props Props) *Node          { return a.Create("Slider", props) }
func (a *App) Textarea(props Props) *Node        { return a.Create("Textarea", props) }
func (a *App) Image(props Props) *Node           { return a.Create("Image", props) }
func (a *App) MenuBar(props Props) *Node         { return a.Create("MenuBar", props) }
func (a *App) Dialog(props Props) *Node          { return a.Create("Dialog", props) }
func (a *App) VirtualList(props Props) *Node     { return a.Create("VirtualList", props) }
func (a *App) Table(props Props) *Node           { return a.Create("Table", props) }
func (a *App) Tabs(props Props) *Node            { return a.Create("Tabs", props) }
func (a *App) Tab(props Props) *Node             { return a.Create("Tab", props) }
func (a *App) Alert(props Props) *Node           { return a.Create("Alert", props) }
func (a *App) Badge(props Props) *Node           { return a.Create("Badge", props) }
func (a *App) Tag(props Props) *Node             { return a.Create("Tag", props) }
func (a *App) Kbd(props Props) *Node             { return a.Create("Kbd", props) }
func (a *App) Progress(props Props) *Node        { return a.Create("Progress", props) }
func (a *App) Spinner(props Props) *Node         { return a.Create("Spinner", props) }
func (a *App) Skeleton(props Props) *Node        { return a.Create("Skeleton", props) }
func (a *App) Rating(props Props) *Node          { return a.Create("Rating", props) }
func (a *App) Pagination(props Props) *Node      { return a.Create("Pagination", props) }
func (a *App) Collapsible(props Props) *Node     { return a.Create("Collapsible", props) }
func (a *App) Accordion(props Props) *Node       { return a.Create("Accordion", props) }
func (a *App) Avatar(props Props) *Node          { return a.Create("Avatar", props) }
func (a *App) Stepper(props Props) *Node         { return a.Create("Stepper", props) }
func (a *App) NumberInput(props Props) *Node     { return a.Create("NumberInput", props) }
func (a *App) OtpInput(props Props) *Node        { return a.Create("OtpInput", props) }
func (a *App) Form(props Props) *Node            { return a.Create("Form", props) }
func (a *App) Field(props Props) *Node           { return a.Create("Field", props) }
func (a *App) GroupBox(props Props) *Node        { return a.Create("GroupBox", props) }
func (a *App) DescriptionList(props Props) *Node { return a.Create("DescriptionList", props) }
func (a *App) StatusBar(props Props) *Node       { return a.Create("StatusBar", props) }
func (a *App) Toolbar(props Props) *Node         { return a.Create("Toolbar", props) }
func (a *App) Resizable(props Props) *Node       { return a.Create("Resizable", props) }
func (a *App) Calendar(props Props) *Node        { return a.Create("Calendar", props) }
func (a *App) Tree(props Props) *Node            { return a.Create("Tree", props) }
func (a *App) Diff(props Props) *Node            { return a.Create("Diff", props) }
func (a *App) Carousel(props Props) *Node        { return a.Create("Carousel", props) }
func (a *App) Sidebar(props Props) *Node         { return a.Create("Sidebar", props) }
func (a *App) Chart(props Props) *Node           { return a.Create("Chart", props) }
func (a *App) Canvas(props Props) *Node          { return a.Create("Canvas", props) }
func (a *App) Command(props Props) *Node         { return a.Create("Command", props) }
func (a *App) Settings(props Props) *Node        { return a.Create("Settings", props) }
func (a *App) Toggle(props Props) *Node          { return a.Create("Toggle", props) }
func (a *App) DropdownButton(props Props) *Node  { return a.Create("DropdownButton", props) }
func (a *App) Label(props Props) *Node           { return a.Create("Label", props) }
func (a *App) Empty(props Props) *Node           { return a.Create("Empty", props) }
func (a *App) Shimmer(props Props) *Node         { return a.Create("Shimmer", props) }
func (a *App) InputGroup(props Props) *Node      { return a.Create("InputGroup", props) }
func (a *App) Clipboard(props Props) *Node       { return a.Create("Clipboard", props) }
func (a *App) AlertDialog(props Props) *Node     { return a.Create("AlertDialog", props) }
func (a *App) Sheet(props Props) *Node           { return a.Create("Sheet", props) }
func (a *App) Popover(props Props) *Node         { return a.Create("Popover", props) }
func (a *App) HoverCard(props Props) *Node       { return a.Create("HoverCard", props) }
