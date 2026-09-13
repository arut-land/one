# Fluent 2 Component Reference

All components import from `@fluentui/react-components`. Wrap application root in `<FluentProvider theme={webLightTheme}>`.

## Table of Contents

- [Actions & Commands](#actions--commands)
- [Inputs & Forms](#inputs--forms)
- [Layout & Containers](#layout--containers)
- [Data Display](#data-display)
- [Navigation](#navigation)
- [Feedback & Status](#feedback--status)
- [Overlays & Surfaces](#overlays--surfaces)
- [Utilities](#utilities)

---

## Actions & Commands

### Button

```jsx
<Button appearance="primary">Primary</Button>
<Button appearance="secondary">Secondary</Button>
<Button appearance="subtle">Subtle</Button>
<Button appearance="transparent">Transparent</Button>
<Button appearance="outline">Outline</Button>
<Button icon={<AddRegular />}>With Icon</Button>
<Button icon={<AddRegular />} />  {/* Icon-only */}
<Button size="small" | "medium" | "large" />
<Button shape="rounded" | "circular" | "square" />
<Button disabled />
<Button disabledFocusable />  {/* Still focusable for a11y */}
```

Variants: `CompoundButton` (title + description), `SplitButton` (action + menu), `ToggleButton` (on/off state), `MenuButton` (triggers menu).

### Link

```jsx
<Link href="https://example.com">External link</Link>
<Link as="button" onClick={handleClick}>Button link</Link>
<Link appearance="subtle">Subtle link</Link>
<Link inline>Inline within text</Link>
```

### Toolbar

```jsx
<Toolbar>
  <ToolbarButton icon={<BoldRegular />}>Bold</ToolbarButton>
  <ToolbarDivider />
  <ToolbarToggleButton icon={<ItalicRegular />} />
  <Menu>
    <MenuTrigger><ToolbarButton icon={<MoreHorizontalRegular />} /></MenuTrigger>
    <MenuPopover><MenuList>...</MenuList></MenuPopover>
  </Menu>
</Toolbar>
```

---

## Inputs & Forms

### Input

```jsx
<Input placeholder="Enter text" />
<Input type="password" />
<Input contentBefore={<SearchRegular />} />
<Input contentAfter={<DismissRegular />} />
<Input size="small" | "medium" | "large" />
<Input appearance="outline" | "underline" | "filled-darker" | "filled-lighter" />
<Input disabled />
```

### Textarea

```jsx
<Textarea placeholder="Enter long text" resize="both" />
<Textarea resize="vertical" | "horizontal" | "both" | "none" />
```

### Select

```jsx
<Select>
  <option>Option A</option>
  <option>Option B</option>
</Select>
```

### Combobox / Dropdown

```jsx
<Combobox placeholder="Search or select">
  <Option>Apple</Option>
  <Option>Banana</Option>
</Combobox>

<Dropdown placeholder="Choose an option">
  <Option>Red</Option>
  <Option>Blue</Option>
</Dropdown>
```

Both support `multiselect`, `freeform` (Combobox only), `<OptionGroup>`.

### Checkbox

```jsx
<Checkbox label="Accept terms" />
<Checkbox checked="mixed" />  {/* Indeterminate */}
<Checkbox shape="circular" />
<Checkbox labelPosition="before" />
```

### RadioGroup

```jsx
<RadioGroup>
  <Radio value="a" label="Option A" />
  <Radio value="b" label="Option B" />
</RadioGroup>
<RadioGroup layout="horizontal" />
```

### Switch

```jsx
<Switch label="Enable feature" />
<Switch labelPosition="before" | "above" | "after" />
```

### Slider

```jsx
<Slider min={0} max={100} defaultValue={50} />
<Slider step={10} />
<Slider vertical />
```

### SpinButton

```jsx
<SpinButton min={0} max={100} defaultValue={0} step={1} />
```

### Field (wrapper for labels + validation)

```jsx
<Field label="Email" validationState="error" validationMessage="Required">
  <Input />
</Field>
<Field label="Name" required hint="As it appears on your ID">
  <Input />
</Field>
```

---

## Layout & Containers

### Card

```jsx
<Card>
  <CardHeader
    image={<Avatar />}
    header={<Body1><b>Title</b></Body1>}
    description={<Caption1>Description</Caption1>}
    action={<Button appearance="transparent" icon={<MoreHorizontalRegular />} />}
  />
  <CardPreview>
    <img src="preview.png" alt="Preview" />
  </CardPreview>
  <CardFooter>
    <Button>Action</Button>
  </CardFooter>
</Card>
<Card appearance="filled" | "filled-alternative" | "outline" | "subtle" />
<Card size="small" | "medium" | "large" />
<Card orientation="horizontal" />
```

### Divider

```jsx
<Divider />
<Divider appearance="brand" | "strong" | "subtle" />
<Divider vertical />
<Divider inset />
<Divider>Section Title</Divider>  {/* With label */}
```

### Dialog

```jsx
<Dialog>
  <DialogTrigger disableButtonEnhancement>
    <Button>Open Dialog</Button>
  </DialogTrigger>
  <DialogSurface>
    <DialogBody>
      <DialogTitle>Title</DialogTitle>
      <DialogContent>Content here</DialogContent>
      <DialogActions>
        <DialogTrigger disableButtonEnhancement>
          <Button appearance="secondary">Close</Button>
        </DialogTrigger>
        <Button appearance="primary">Confirm</Button>
      </DialogActions>
    </DialogBody>
  </DialogSurface>
</Dialog>
```

Modal vs non-modal: `<Dialog modalType="modal" | "non-modal" | "alert" />`

### Drawer

```jsx
<OverlayDrawer position="start" | "end" open={isOpen} onOpenChange={setOpen}>
  <DrawerHeader><DrawerHeaderTitle>Title</DrawerHeaderTitle></DrawerHeader>
  <DrawerBody>Content</DrawerBody>
</OverlayDrawer>

<InlineDrawer position="start" open={isOpen}>
  {/* Same structure */}
</InlineDrawer>
```

### Popover

```jsx
<Popover>
  <PopoverTrigger disableButtonEnhancement>
    <Button>Info</Button>
  </PopoverTrigger>
  <PopoverSurface>
    Popover content
  </PopoverSurface>
</Popover>
```

### Tooltip

```jsx
<Tooltip content="Helpful tip" relationship="label">
  <Button icon={<InfoRegular />} />
</Tooltip>
```

`relationship`: `"label"` (replaces accessible name), `"description"` (supplemental).

---

## Data Display

### Table / DataGrid

```jsx
<Table>
  <TableHeader>
    <TableRow>
      <TableHeaderCell>Name</TableHeaderCell>
      <TableHeaderCell>Status</TableHeaderCell>
    </TableRow>
  </TableHeader>
  <TableBody>
    <TableRow>
      <TableCell>Item 1</TableCell>
      <TableCell><Badge appearance="filled" color="success">Active</Badge></TableCell>
    </TableRow>
  </TableBody>
</Table>
```

`DataGrid` adds sorting, selection, and keyboard navigation out of the box. Use `DataGrid` + `DataGridHeader` + `DataGridBody` + `DataGridRow` + `DataGridCell`.

### Avatar / AvatarGroup

```jsx
<Avatar name="John Doe" />
<Avatar image={{ src: "photo.jpg" }} />
<Avatar size={24 | 28 | 32 | 36 | 40 | 48 | 56 | 64 | 72 | 96 | 120 | 128} />
<Avatar color="brand" | "colorful" | "neutral" />

<AvatarGroup>
  <AvatarGroupItem name="Alice" />
  <AvatarGroupItem name="Bob" />
  <AvatarGroupPopover>
    <AvatarGroupItem name="Charlie" />
  </AvatarGroupPopover>
</AvatarGroup>
```

### Badge / CounterBadge / PresenceBadge

```jsx
<Badge appearance="filled" | "ghost" | "outline" | "tint">99</Badge>
<Badge color="brand" | "danger" | "important" | "informative" | "severe" | "subtle" | "success" | "warning" />
<Badge size="tiny" | "extra-small" | "small" | "medium" | "large" | "extra-large" />
<Badge shape="rounded" | "circular" | "square" />

<CounterBadge count={5} />
<CounterBadge dot />

<PresenceBadge status="available" | "busy" | "do-not-disturb" | "away" | "offline" | "out-of-office" | "blocked" />
```

### Tag / TagGroup

```jsx
<TagGroup onDismiss={handleDismiss}>
  <Tag dismissible dismissIcon={{ "aria-label": "remove" }}>Tag 1</Tag>
  <Tag>Tag 2</Tag>
</TagGroup>
<InteractionTag>
  <InteractionTagPrimary>Clickable Tag</InteractionTagPrimary>
</InteractionTag>
```

### Accordion

```jsx
<Accordion>
  <AccordionItem value="1">
    <AccordionHeader>Section 1</AccordionHeader>
    <AccordionPanel>Content 1</AccordionPanel>
  </AccordionItem>
</Accordion>
<Accordion collapsible />      {/* All can close */}
<Accordion multiple />         {/* Multiple open */}
```

### Tree

```jsx
<Tree>
  <TreeItem itemType="branch">
    <TreeItemLayout>Parent</TreeItemLayout>
    <Tree>
      <TreeItem itemType="leaf">
        <TreeItemLayout>Child</TreeItemLayout>
      </TreeItem>
    </Tree>
  </TreeItem>
</Tree>
```

### Persona

```jsx
<Persona
  name="Jane Doe"
  secondaryText="Software Engineer"
  tertiaryText="Available"
  avatar={{ color: "colorful" }}
  size="medium"
/>
```

---

## Navigation

### TabList

```jsx
<TabList selectedValue={selectedTab} onTabSelect={(_, data) => setTab(data.value)}>
  <Tab value="tab1">Tab 1</Tab>
  <Tab value="tab2">Tab 2</Tab>
  <Tab value="tab3" icon={<SettingsRegular />}>Settings</Tab>
</TabList>
<TabList appearance="subtle" | "transparent" />
<TabList size="small" | "medium" | "large" />
<TabList vertical />
```

### Breadcrumb

```jsx
<Breadcrumb>
  <BreadcrumbItem><BreadcrumbButton>Home</BreadcrumbButton></BreadcrumbItem>
  <BreadcrumbDivider />
  <BreadcrumbItem><BreadcrumbButton>Category</BreadcrumbButton></BreadcrumbItem>
  <BreadcrumbDivider />
  <BreadcrumbItem><BreadcrumbButton current>Item</BreadcrumbButton></BreadcrumbItem>
</Breadcrumb>
```

---

## Feedback & Status

### MessageBar

```jsx
<MessageBar intent="info" | "success" | "warning" | "error">
  <MessageBarBody>
    <MessageBarTitle>Title</MessageBarTitle>
    Message content
  </MessageBarBody>
  <MessageBarActions containerAction={<Button appearance="transparent" icon={<DismissRegular />} />}>
    <Button>Action</Button>
  </MessageBarActions>
</MessageBar>
```

### Toast

```jsx
const { dispatchToast } = useToastController(toasterId);
dispatchToast(
  <Toast>
    <ToastTitle>Success</ToastTitle>
    <ToastBody>Item saved.</ToastBody>
  </Toast>,
  { intent: "success", position: "bottom-end" }
);
// Wrap app with <Toaster toasterId={toasterId} />
```

### ProgressBar

```jsx
<ProgressBar value={0.5} />           {/* Determinate */}
<ProgressBar />                        {/* Indeterminate */}
<ProgressBar thickness="large" />
<ProgressBar color="brand" | "success" | "warning" | "error" />
```

### Spinner

```jsx
<Spinner size="tiny" | "extra-small" | "small" | "medium" | "large" | "extra-large" | "huge" />
<Spinner label="Loading..." labelPosition="below" />
```

### Skeleton

```jsx
<Skeleton>
  <SkeletonItem shape="rectangle" | "circle" | "square" />
  <SkeletonItem size={16 | 20 | 24 | 28 | 32 | 36 | 40 | 48 | 56 | 64 | 96 | 120 | 128} />
</Skeleton>
```

---

## Overlays & Surfaces

### Menu

```jsx
<Menu>
  <MenuTrigger disableButtonEnhancement>
    <Button>Menu</Button>
  </MenuTrigger>
  <MenuPopover>
    <MenuList>
      <MenuItem icon={<EditRegular />}>Edit</MenuItem>
      <MenuItem icon={<DeleteRegular />}>Delete</MenuItem>
      <MenuDivider />
      <MenuGroup>
        <MenuGroupHeader>More</MenuGroupHeader>
        <MenuItem>Settings</MenuItem>
      </MenuGroup>
      <MenuItemCheckbox name="options" value="bold">Bold</MenuItemCheckbox>
      <MenuItemRadio name="size" value="small">Small</MenuItemRadio>
    </MenuList>
  </MenuPopover>
</Menu>
```

Supports nested submenus via `<Menu>` inside `<MenuItem>`.

### Overflow

For responsive toolbars that collapse items into an overflow menu:

```jsx
<Overflow>
  <OverflowItem id="item1"><Button>Item 1</Button></OverflowItem>
  <OverflowItem id="item2"><Button>Item 2</Button></OverflowItem>
  <OverflowItem id="item3"><Button>Item 3</Button></OverflowItem>
  <Menu>
    <MenuTrigger disableButtonEnhancement>
      <Button icon={<MoreHorizontalRegular />} />
    </MenuTrigger>
    <MenuPopover>
      <MenuList>
        {overflowItems.map(item => <MenuItem key={item}>{item}</MenuItem>)}
      </MenuList>
    </MenuPopover>
  </Menu>
</Overflow>
```

---

## Utilities

### Text

```jsx
import { Text, Body1, Body1Strong, Caption1, Title1, Display } from "@fluentui/react-components";

<Text>Default text</Text>
<Text size={500} weight="semibold">Custom</Text>
<Body1>Body text</Body1>
<Caption1>Small caption</Caption1>
<Title1>Page title</Title1>
```

### Icons

Use `@fluentui/react-icons`:

```bash
npm install @fluentui/react-icons
```

```jsx
import { AddRegular, AddFilled, bundleIcon } from "@fluentui/react-icons";

const Add = bundleIcon(AddFilled, AddRegular); // Filled on interaction, Regular at rest

<Button icon={<AddRegular />}>Add</Button>
```

Icon naming: `{Name}{Style}` where Style = `Regular`, `Filled`, `Light`, `Color`.
Sizes: Icons come in 12, 16, 20, 24, 28, 32, 48 variants (e.g., `Add20Regular`, `Add24Filled`).

### mergeClasses

```jsx
import { mergeClasses } from "@fluentui/react-components";

// Correct — deduplicates and resolves atomic CSS
<div className={mergeClasses(styles.base, isActive && styles.active)} />

// Incorrect — never concatenate
<div className={styles.base + " " + styles.active} />
```

### shorthands

```jsx
import { shorthands } from "@fluentui/react-components";

const useStyles = makeStyles({
  root: {
    ...shorthands.borderRadius(tokens.borderRadiusMedium),
    ...shorthands.border(tokens.strokeWidthThin, "solid", tokens.colorNeutralStroke1),
    ...shorthands.padding(tokens.spacingVerticalS, tokens.spacingHorizontalM),
    ...shorthands.gap(tokens.spacingHorizontalS),
  },
});
```

Note: `shorthands` is being deprecated in newer versions as Griffel adds native shorthand support. Check current docs.
