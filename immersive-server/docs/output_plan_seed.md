Advanced Output in Resolume Arena: A Comprehensive Tutorial

Introduction: Resolume Arena’s Advanced Output is a powerful mapping interface that lets you control exactly how your visuals are sent to various displays and devices. Using the Advanced Output, you can manage everything from standard projectors and LED walls to DMX-controlled lighting fixtures and virtual outputs (like Syphon/Spout feeds)
resolume.com
. In practical terms, this is where the magic happens for multi-screen shows or projection mapping setups
resolume.com
. With Advanced Output, you can split your composition into slices, warp and blend them to fit physical surfaces, route specific content to different outputs, and calibrate colors and brightness per output. This tutorial-style breakdown will guide you through every button, setting, and feature in Resolume’s Advanced Output, covering the interface layout, input slicing, output mapping, warping, masking, edge blending, color adjustments, display configurations, previews, and specialized tools like DMX pixel mapping. Let’s dive in!

Accessing Advanced Output & Interface Overview

To open the Advanced Output window in Resolume Arena, go to Output > Advanced in the menu. The Advanced Output interface is divided into three main areas: a Screens list on the left, a central preview canvas, and a Properties panel on the right
resolume.com
resolume.com
. At the top-left of the window, there is a Preset dropdown where you can save and load output configurations, which is useful for quickly switching between different venue setups
resolume.com
.

The Resolume Arena Advanced Output window (Input Selection tab). Here, “Screen 1” is assigned to a physical display, with three slices (“Center Rotated”, “Horizontal Strip”, and “Vertical Strip”). “Screen 2” is a Syphon output (1920×1080) containing a triangular slice and a full-screen slice. The central canvas shows a preview of the composition with slice outlines, and the right panel displays properties for the selected screen (including device selection, resolution, and color adjustments).
resolume.com
resolume.com

Screens List (Left Panel): On the left, you’ll see a list of Screens, each representing an output destination
resolume.com
. By default, you start with one Screen (Screen 1) containing one slice, but you can add more via the “+” button. Each Screen can be assigned to a physical or virtual output. The name of the screen and its assigned device (display, NDI, etc.) are shown in the list, and you can double-click the name to rename a screen
resolume.com
. You can also toggle a screen on/off (to temporarily disable output) and fold/unfold a screen to hide its slice list without disabling it
resolume.com
. (Toggling is useful if you need to blackout one output during a show or troubleshoot, while folding keeps the output active but collapses the view when working with many slices.) Each screen’s entry will list its Slices as child items. Right-clicking on a Screen opens a menu to assign it to a different output device.

Properties Panel (Right Panel): When you select a Screen or slice, the right-side panel shows detailed properties. For a Screen, you’ll find the Device dropdown (to pick which physical display, projector, or other output this screen should use)
resolume.com
, as well as Width and Height fields (the resolution for that screen’s output). You can change a Screen’s resolution here if it’s a virtual output or Spout/Syphon sender (e.g. set a custom size)
resolume.com
resolume.com
. Below that are Output Adjustment controls for the screen: Opacity, Brightness, Contrast, and individual Red/Green/Blue channel gains
resolume.com
. These let you calibrate differences between screens — for example, dim an LED wall while keeping a projector at full brightness, or correct a projector’s color cast by reducing one color channel
resolume.com
. There’s also a Delay setting (0–100 ms) to offset output timing per screen, to compensate for any latency in cables or wireless links
resolume.com
.

Central Canvas and Mode Tabs: The center of the Advanced Output window is the canvas where you configure mappings. It actually has two modes/tabs: Input Selection and Output Transformation. You can switch between these stages to first define what part of your composition each slice shows, and then define how those slices are positioned/warped on the outputs. The Input Selection stage shows a preview of your full composition (usually with the live output or a test card as the background)
resolume.com
 and allows you to define slices (think of slicing up your composition like pieces of a pie)
resolume.com
. The Output Transformation stage (Arena only) is where you take those slices and move or warp them to fit your physical screens or surfaces
resolume.com
resolume.com
. We will examine each stage in detail.

Tip: Before configuring Advanced Output, ensure your operating system is set to extended desktop mode with all external displays active (not mirrored). Resolume relies on the OS to provide display outputs; if your computer doesn’t recognize a projector or monitor, it won’t appear in Resolume’s device list
resolume.com
. On Windows, check Settings > System > Display; on macOS, use Displays preferences and set the output as an extended display
resolume.com
resolume.com
. If you accidentally send the output fullscreen to your main interface display (hiding the Resolume UI), press CTRL+SHIFT+D (CMD+SHIFT+D on Mac) to quickly disable all outputs and return to the interface
resolume.com
resolume.com
.

Setting Up Screens and Output Routing

Each Screen in Advanced Output represents one output signal from Resolume. You can have multiple screens to send different parts of your visuals to different outputs simultaneously
resolume.com
resolume.com
. Common scenarios include spanning a wide image across two projectors, sending distinct content to separate LED walls, or mixing various output types (projectors, LED processors, capture cards, virtual feeds) in one setup. Resolume Arena’s Advanced Output is flexible enough to handle all of these at once
resolume.com
resolume.com
.

Adding and Assigning Screens: To add another output screen, click the “+” menu in the Screens list and choose Add Screen. The new screen will appear in the list (e.g. “Screen 2”) and by default is set to Virtual. To assign it to a real output, right-click the screen (or use the Device dropdown on the right panel) and select from the list of available outputs. Resolume will list all Connected Displays by name and resolution (as provided by the OS)
resolume.com
. You’ll also see options for Spout/Syphon, NDI, and any Capture/Playback cards (like Blackmagic, AJA, Datapath) if installed
resolume.com
resolume.com
. Selecting a device instantly routes that screen’s output there – for example, choose the projector on HDMI for Screen 1, your LED processor on DisplayPort for Screen 2, etc. (Remember, a given physical output can only be assigned to one Resolume screen at a time – if you try to use the same display for two screens, Resolume will automatically set the previous one back to virtual to avoid conflicts
resolume.com
.) The Screens list will show the chosen device under each screen’s name for confirmation
resolume.com
.

Output Types: Resolume supports a variety of output types in Advanced Output:

Physical Displays (Monitors/Projectors): Any monitor or projector connected and recognized by the OS will appear by name/resolution. Simply assign your screen to it. (If a connected device isn’t showing up in Resolume, ensure your system detects it and it’s not set to mirror another display
resolume.com
.)

LED Processors / Playback Cards:** If you use specialized hardware (Datapath, Blackmagic, etc.), those outputs appear in the list as well. Selecting one may reveal extra settings (e.g. selecting card output port, video format or frame rate)
resolume.com
. For smooth playback, match the output format’s frame rate to your content, composition, and display’s refresh (avoiding, say, a 50i output to a 60Hz display)
resolume.com
.

Syphon (Mac) / Spout (Windows): These are virtual texture-sharing outputs. Assigning a screen to Syphon/Spout makes Resolume broadcast that screen’s output as a live texture other applications can receive
resolume.com
resolume.com
. The screen’s name becomes the Syphon/Spout server name (e.g. “Screen 2” or a custom name you give it)
resolume.com
. You can also set a custom resolution for Syphon/Spout outputs via the screen’s Width/Height if needed
resolume.com
.

NDI (Network Stream): NDI outputs allow sending the screen’s output over a network to other NDI-enabled systems. When you set a screen to NDI, Resolume announces it on the network automatically
resolume.com
resolume.com
. This is useful for offloading a feed to another machine or wireless streaming. (NDI can even send a 1080p feed from a VJ laptop to a main server for further scaling/mapping
resolume.com
.)

Virtual Screens: A Virtual Output means the screen isn’t sent to any external device, but it can be used internally as an intermediate texture
resolume.com
. Other screens’ slices can take a Virtual Screen as their input source (we’ll see this in Slice routing). Virtual outputs enable complex multi-pass mappings – for example, you might map content onto a virtual screen first, then feed that into multiple physical outputs. They have minimal performance cost (0–1 frame delay) but use some GPU resources, so use them when needed, not arbitrarily
resolume.com
. You can set a virtual screen’s resolution via Width/Height as well
resolume.com
.

Once a screen is assigned to a device, you should set its resolution to match the device’s native resolution or the desired output raster. For example, if spanning two HD projectors, Screen 1 might be 1920×1080 on Projector A and Screen 2 also 1920×1080 on Projector B; or if an LED wall is 3840×2160, you’d set the screen’s resolution accordingly before slicing
app.getmxu.com
. This ensures your Output Transformation canvas matches the actual pixel space of the output.

Screen-Level Adjustments: Under each screen’s properties, you have the Opacity/Brightness/Contrast and RGB controls mentioned earlier. Use these to fine-tune the output if needed – for instance, balancing color between projectors (removing a bit of red if one projector has a warmer tint) or dimming one screen’s output relative to another
resolume.com
resolume.com
. Each screen’s opacity can also effectively fade that entire output if you needed to crossfade or blackout an entire screen feed. The Delay per screen is helpful if one output path has more latency (for example, a wireless sender or an LED processor might introduce a slight delay; you can delay other screens to sync up)
resolume.com
.

Hiding and Folding Screens: In the screens list, you can click the little eye icon (or toggle) next to a screen to disable/enable that screen’s output entirely
resolume.com
. This is useful if you want to turn off an LED wall during a part of the show, or quickly check outputs one by one (each disabled screen stops sending content until re-enabled). Folding (clicking the arrow next to the screen name) collapses the slice list for that screen without turning it off
resolume.com
, which helps declutter the view when you have many screens and slices.

Saving and Switching Configurations: The Preset dropdown at the top of the Advanced Output window lets you save the current screen setup as a preset and load presets. This way, you can prepare different configurations for different venues or stage designs and recall them instantly
resolume.com
. The presets are stored as XML files (and can be shared between machines, even Mac <-> PC)
resolume.com
resolume.com
. For example, you might have a “Club Setup” preset with two LED panels and a “Festival Stage” preset with three projectors – you can swap presets without rebuilding the mapping from scratch. (Note: Loading a new screen preset will reconfigure outputs and can cause a brief display interruption, so do this during production prep, not mid-show.) Also, if you need to locate the preset file (to back up or copy to another computer), use the “Reveal in Finder/Explorer” option in the preset menu
resolume.com
.

Preparation without Connected Displays: If you open Advanced Output on a system with no external displays connected (e.g., pre-production on your laptop), Resolume will start with a single Virtual Screen by default
resolume.com
. You can manually set that screen’s Width and Height to the resolution you will use later in the venue
resolume.com
. This allows you to set up slices and mapping in advance. Later, when you’re at the venue with actual hardware, just right-click and reassign that Virtual Screen to the real output device – all your slices and mappings will remain intact at the correct resolution
resolume.com
. You can create as many virtual screens as needed via the “+” menu, allowing you to fully prepare multi-screen setups offline
resolume.com
.

Input Selection Stage: Slices and Source Content

The Input Selection stage is where you define slices, which are essentially subregions of your composition (or specific layers) that you want to send to outputs. Think of your composition as a big image or “pie”, and slices as pieces cut from that pie
resolume.com
. By creating slices, you control which part of the composition (or which layer) is shown on each output. For instance, you could slice a 3840×1080 composition down the middle, sending the left half to Screen 1 (projector 1) and the right half to Screen 2 (projector 2). But you’re not limited to simple halves – slices can be any size or shape to accommodate different aspect ratios and resolutions
resolume.com
.

When you open Advanced Output for the first time (or create a new screen), Resolume generates one default slice per screen that covers the entire composition. This means by default each screen would show the whole comp output. From there, you can add or edit slices as needed:

Creating and Selecting Slices: To add a new slice, select the screen in the left list and click the + menu (next to Slices) and choose Add Slice. You can create as many slices as needed – Resolume imposes no practical limit (technically up to 1.15×10^18 slices, far beyond any real use!)
resolume.com
. The new slice will appear under that screen. You can select a slice by clicking its name in the left list or by clicking its outline on the preview canvas. The selected slice is highlighted, and its input mapping controls appear on the right panel.

Positioning and Resizing Slices: On the Input Selection canvas (which shows your composition), slices appear as rectangles (by default) with handles. You can drag a slice to reposition which part of the comp it covers, drag the edges/corners to resize it, or use the rotate handle (often a small circle at a corner) to rotate the slice’s input area
resolume.com
. For example, if you have a rotated LED panel, you can rotate the slice so that the content for that panel is taken from a rotated section of the comp
resolume.com
. As you drag and resize, slices will snap to the comp edges and to each other for alignment – hold CTRL while dragging to temporarily disable snapping for fine adjustments
resolume.com
resolume.com
.

You can also precisely adjust slice properties using the numeric fields on the right panel: X, Y position, Width, Height, and Rotation. You can type exact numbers for pixel-perfect accuracy. Handy trick: these fields support simple math, so you can enter expressions (e.g., enter the current width followed by “/3” to set the slice to exactly one-third its size)
resolume.com
. Nudge keys: with a slice selected, the arrow keys move it by 1 pixel (hold Shift for 10px increments)
resolume.com
 – useful for precise alignment.

Input Source (Composition vs Layer routing): By default, each slice’s input is the full Composition output. However, you can change a slice’s Input Source to a specific layer, group, or other source using the dropdown in the slice’s properties
resolume.com
. This is an extremely powerful feature called Layer-to-Slice Routing
resolume.com
. For example, if you have six screens each needing different content, you could keep your composition at 1920×1080 and have six layers, each routed directly to its own slice (and screen)
resolume.com
. This way, each layer plays content only for its intended output, which is efficient and avoids needing a gigantic comp resolution for a multi-screen setup
resolume.com
. Once a slice is set to a specific layer, it will only show that layer’s visuals (independent of the composition’s main mix). You can even still warp/transform that layer’s output separately in the output stage.

Ignore Opacity / Bypass: When using layer (or group) routing, you’ll notice two toggle options in the slice properties: “Ignore Opacity & Solo/Bypass”
resolume.com
resolume.com
. Enabling these means the slice will ignore the layer’s opacity fader and whether the layer is bypassed or soloed in the composition. In practice, this lets you remove the layer from the main composition but still have it visible in the slice’s output
resolume.com
. For instance, you might want a layer to only appear on a specific LED screen and not in your main mix – you would route that layer to a slice and then bypass it in the comp (and turn off the Input Bypass toggle on the slice). The layer won’t render in the comp preview, but it will still show up on that slice’s output
resolume.com
. This is very useful for output-exclusive content like a logo or text on a separate output.

Important: Using layer-to-slice routing changes how blending works. When a layer is routed directly to a slice, it bypasses the composition’s layer blending and effects chain. In the final output, slices are simply stacked in the order they appear (with transparency)
resolume.com
resolume.com
. In other words, blend modes between layers do not carry through when those layers are individually routed to slices
resolume.com
resolume.com
. If you need to use complex layer blend modes or composition-wide effects, you should use the Composition as the slice input (or use the Slice Transform effect within the comp) rather than routing individual layers. Layer routing is best for when you want completely separate content per output or when you’re sure you won’t need to composite that layer with others via blend modes
resolume.com
resolume.com
.

Duplicate and Move Slices: You can duplicate an existing slice (right-click a slice > Duplicate) to create a copy, which you can then adjust independently
resolume.com
. You can also drag slices between screens in the list if needed – for instance, if you accidentally created a slice under Screen 1 but meant it for Screen 2, just drag it over. (Note that a screen must always have at least one slice; you cannot drag away the last slice from a screen or it will refuse the move
resolume.com
.)

Slice Presets (Right-Click Menu): Right-clicking a slice (either in the list or on the canvas) brings up some useful actions and presets
resolume.com
. For example, “Match Output Shape” is a preset that instantly resizes the slice’s input area to match the shape/aspect it has in the Output stage
resolume.com
. This is great for LED pixel maps: you can first position/warp a slice on output, then use Match Output Shape so the input slice perfectly corresponds to that shape (ensuring pixel-perfect mapping)
resolume.com
. Other options can include splitting a slice (dividing one slice into multiple equal parts), mirroring slices, etc., depending on Resolume version.

Multiple Slices & Multi-Select: A single screen can contain many slices. These slices could cover distinct regions of the comp or even overlapping regions. If slices on the same screen overlap in the input selection, their content will overlap when sent to that output (the one higher in the list will appear on top of those below in overlapping areas, unless “Black BG” is used – more on that later). You can also select multiple slices at once to move/scale them together. Drag a selection box around slices on the canvas or shift-click their names to multi-select
resolume.com
. A bounding box will appear around the group, and you can manipulate this box to transform all selected slices together
resolume.com
. The right panel will show the aggregate X, Y, width, height of the group; adjusting these values or rotating will affect the whole group (note the numeric values refer to the bounding box, not individual slices)
resolume.com
.

Slice Masks (Cutting Shapes out of Slices)

By default, slices are rectangular. However, Resolume Arena allows you to apply Slice Masks to carve a slice into arbitrary shapes (available only in Arena, not Avenue)
resolume.com
resolume.com
. A slice mask is essentially a polygonal mask that sits on top of a slice’s input, allowing only part of that rectangle to actually show content. This is useful when you need non-rectangular outputs but still want to work with a simple rectangular slice region for content.

To add a mask to a slice, select the slice in Input Selection and use the mask tools on the right side. You’ll find preset shapes (like circle, triangle, etc.) and a Pen/Pencil tool for freeform masks
resolume.com
. Clicking a preset immediately adds that mask shape to the slice; clicking the pencil lets you draw a custom shape. Once a mask is added, it will appear as a colored outline over the slice on the canvas. By default, the mask cuts the slice to that shape (everything outside the mask is hidden)
resolume.com
. You can invert the mask via the Invert toggle to instead remove the inside and keep everything outside (effectively turning it into a crop)
resolume.com
. This invert toggle is handy if you want to punch a hole in a slice rather than define a visible area.

When a slice has a mask, you’ll see two mode buttons at the top of the Input Selection canvas: Transform and Edit Points (specific to the mask)
resolume.com
.

Mask Transform Mode: In this mode, the mask (the shape) can be moved, scaled, or rotated as a whole, relative to the slice
resolume.com
. The slice itself can also still be moved, but typically you might position the slice first, then adjust the mask. Be careful not to drag the mask entirely off the slice’s area – if the mask no longer overlaps the slice at all, that slice would output nothing (since you’d be masking the entire area away)
resolume.com
.

Mask Edit Points Mode: Switch to Edit Points to tweak the individual vertices of the mask shape
resolume.com
. You can click and drag existing points to new positions. Double-click on the mask’s outline to add a new point, or double-click an existing point to remove it
resolume.com
. This allows you to create a precise custom shape. By default, masks you draw are Bezier curves (smooth curves between points) – you can change a point between Linear and Bezier to get either straight line segments or curved handles on that point
resolume.com
. In Bezier mode, each point shows tangent handles you can drag to adjust the curvature of the mask at that point
resolume.com
. For straight-edged masks, switch all points to linear. This flexibility means you can create almost any shape mask – rounded shapes, star-like shapes, irregular polygons – to tailor the slice’s visible area
resolume.com
resolume.com
.

Tip: When using a mask for mapping a specific physical shape (say a circular screen), it often helps to size the slice itself close to that shape’s bounding box. If you have a huge slice but only mask a small portion of it, the subsequent warping might not behave as intuitively because Resolume still bases the warping on the slice’s full rectangular bounds rather than the mask shape
resolume.com
. So generally, scale your slice to just encompass the area you need, then mask it.

Polygon Slices (Custom-Shaped Slices)

In addition to masking a rectangle, Arena offers Polygon Slices – slices that are inherently multi-point polygons rather than simple rectangles (also Arena-only)
resolume.com
. A polygon slice lets you directly define the shape of the slice itself (whereas a mask affects a rectangular slice). This is the most flexible way to map non-rectangular surfaces, as the slice’s output mapping will conform to a shape you draw.

To create a polygon slice, use the + menu and choose Add Polygon Slice. You can either start with a preset triangle or draw a custom polygon freehand
resolume.com
. When drawing freehand, click on the canvas to place each vertex of the shape; continue clicking for each corner. Double-click the first point or anywhere to close the shape when done
resolume.com
. For example, you could draw a hexagon slice, a star, or trace the outline of a mapping surface.

Once created, Resolume will internally triangulate the polygon slice for warping purposes
resolume.com
. The same vertices you placed in Input Selection will appear in the Output Transformation stage for warping. Note that polygon slices must be non-self-intersecting (you can’t have a shape that overlaps itself); if you do create an impossible shape, Resolume will highlight it in red to indicate it can’t triangulate that geometry
resolume.com
. Unlike masks, polygon slices do not support Bezier curves on points – all edges are straight lines (this keeps the math manageable for real-time warping)
resolume.com
.

Working with a polygon slice is similar to working with a masked slice, except the shape is the slice. In Input Selection, you still have Transform Mode (you can move, scale, rotate the whole polygon slice as one object)
resolume.com
, and Edit Points Mode (adjust individual vertices)
resolume.com
. You can add/remove vertices on a polygon slice in Edit Points mode just like with masks (double-click to add points on the outline)
resolume.com
. Essentially, a polygon slice combines the idea of slice+mask into one – giving you “ultimate control” over the slice shape for complex designs
resolume.com
.

Polygon slices are extremely useful for mapping content to irregular LED panel arrangements or projection surfaces that aren’t rectangular. Each polygon slice’s shape will directly translate to the output mapping.

Slice Ordering and Source Considerations

It’s worth noting how slices composite if they overlap on the same screen:

If slices do not overlap in the Output Transformation (i.e., each slice goes to a distinct area of the physical output), then their order doesn’t matter – they’ll just fill their respective regions.

If slices overlap on the same screen’s output (for example, two slices mapped to partially the same area), the slice higher in the left list will be drawn on top of those below
resolume.com
. This only generally happens if you intentionally overlap them or if using layer routing (which ignores comp blending).

You can force certain slices to block others by enabling Black BG on the top slice – this renders a solid black background behind that slice’s content instead of transparency (more on this option in the Output section)
resolume.com
.

For now, after setting up your slices in Input Selection (sizing and placing them on the comp, and assigning their input sources), you have basically defined what each output will show from the composition. Next, we move to the Output Transformation stage to define how and where those slices appear on the actual outputs.

Output Transformation Stage: Mapping, Warping, and Output Options

The Output Transformation stage is available in Resolume Arena (not in Avenue) and is where you adjust the slices to align with the real-world outputs. While Input Selection chooses the content for each slice, Output Transformation lets you move, scale, rotate, and warp the slices on the final output canvas
resolume.com
resolume.com
. In other words, this is where you make your content fit the physical screens or mapping surfaces.

To switch to Output Transformation, click the Output (or “Output Transformation”) tab at the top of the Advanced Output window. You will see the same screens and slices on the left, but the central canvas now represents the output space for the selected screen (by default)
resolume.com
. For each Screen, the output canvas is typically the resolution of that screen (e.g., if Screen 1 is 1920×1080, you’ll have a 1920×1080 space for Screen 1’s slices). You can select which screen to edit from the left list (each screen’s slices and any output-specific settings will appear when you select it).

Slice Position & Transform (Output Transform Tab)

In the Output Transformation stage, the Transform tab (usually selected by default) allows you to position and scale entire slices in the output. This works much like moving slices in Input Selection, except now you’re moving the output image of the slice rather than the input region. You can drag a slice on the output canvas to the desired location, drag corners to scale it, or use the numeric fields for precise output X, Y, width, height, and rotation
resolume.com
.

For example, if in Input Selection you defined a slice as the bottom-left quarter of the composition, in Output Transformation you might position that slice to the top-left quadrant of a projector’s output. By doing so, you are shuffling pixels around to the correct spot on the physical output
resolume.com
resolume.com
. If you simply scaled or moved slices in Input stage, you’d be altering which part of the comp they take; doing it in Output stage means you keep the same content but display it in a different place on the final screen.

A typical use-case: Imagine a wide composition that feeds two projectors side by side. In Input Selection, you might have two slices each taking half the composition. In Output Transformation, you would take slice 1 and move it to fill projector 1’s entire output, and slice 2 moved to projector 2’s output. If those projectors are aligned to form one big image, you would leave the slices butted against each other. Alternatively, if one projector is mounted rotated or if an LED panel is oriented differently, you can rotate a slice here to compensate (so content appears upright on the physical screen).

Key difference – adjusting a slice in Output Transformation moves the pixels on output rather than selecting different pixels. So, scaling a slice here will stretch or shrink its content on the actual screen, whereas scaling in Input Selection would have captured more or fewer pixels from the comp (without scaling the content itself)
resolume.com
. Both stages work together: Input Selection defines what portion of the media each slice gets, Output stage defines where and how that portion is displayed.

Warping and Geometry Correction (Edit Points Tab)

One of the most powerful features in Output Transformation is the ability to warp slices for projection mapping or aligning with curved/angled surfaces. By switching from the Transform tab to the Edit Points (Warping) tab, you can manipulate the mesh of each slice.

When Edit Points mode is active, each slice will display a set of control points on the output canvas. For a regular (rectangular) slice, you’ll initially see four large corner points (one at each corner of the slice) and smaller points along the edges or inside depending on warping mode
resolume.com
resolume.com
. There are a couple of warping modes to understand:

Perspective Warping: This mode uses the four big corner points of a slice to adjust for perspective distortion
resolume.com
. When you move these corner handles, Resolume will distort the slice in a way that preserves perspective scaling. This is ideal when projecting onto a surface at an angle or onto 3D objects where parts of the surface recede from view. For example, if projecting onto the side of a cube or a wall at an oblique angle, perspective warping ensures that a square image looks square on that angled surface (compensating for the projector’s skew)
resolume.com
resolume.com
. In practice, you’d drag the corners of your slice so they align with the corners of the object or screen in the projection. The content will stretch appropriately such that it appears correct to the viewer. For most simple mappings on flat surfaces (especially rectangles at an angle), perspective warping with the four corners is what you want
resolume.com
.

Note: Perspective warping is only available for normal rectangular slices (not for Polygon slices)
resolume.com
. Polygon slices can still be warped, but only using linear point warping (explained next), because the math for true perspective on arbitrary shapes is much more complex.

Linear Grid Warping: In addition to the four main corners, you’ll see smaller square points near the edges of the slice – these allow linear warping of the edges (non-perspective)
resolume.com
. By default, each edge might have one midpoint handle (making it a 2×2 grid: four corners + one midpoint on each side). You can add more points by clicking the “+” handles or via shortcuts, subdividing the slice into a finer grid horizontally or vertically
resolume.com
. These linear warp points let you bend and pull the slice’s output in a mesh-like fashion. Unlike perspective warp, these do not preserve uniform scale; they simply distort the image to match arbitrary shapes. This is useful for mapping onto curved surfaces or any surface that isn’t a simple planar rectangle. You can move any of these points and the image will linearly interpolate the distortion.

By default, the warping points function in Linear mode, meaning the image between points is stretched along straight lines. However, you can switch the warp grid to Bezier mode: each point will then get Bezier curve handles, allowing you to create smooth curved distortions instead of straight-line folds
resolume.com
resolume.com
. For example, you could warp an image into a smoothly curving shape (like mapping onto a cylindrical screen or a wavy surface) by adjusting the Bezier handles to curve the grid lines
resolume.com
. You can toggle a given warp point between Linear and Bezier if you only want some sections curved.

Tip: You don’t have to choose one or the other – you can use both perspective and linear warping together for best results. Often a workflow is: first, use the big corner points to do a rough perspective alignment of the slice to the projection surface, ensuring correct overall scaling; then use the smaller grid points (with linear or bezier) to fine-tune any curvatures or non-linear deviations
resolume.com
. This way you maintain correct perspective (so the content’s proportions are right) while also fitting any irregularities of the screen shape
resolume.com
.

Warping Polygon Slices: For polygon slices, the warping works slightly differently. A polygon slice will have control points at each of its vertices (the shape you drew). In Output Transformation’s Edit Points mode, you can drag those vertices to adjust how the slice fits on the output. Essentially, Resolume treats each polygon slice as a mesh of triangles (triangulated from your shape)
resolume.com
. You can move each vertex to line up with the corresponding points on the physical object. You cannot do perspective warping or bezier curves on a polygon slice; you simply move its points (linear mapping)
resolume.com
. This still offers a lot of control, especially if your polygon has many points – you can contour the content to complex shapes. The fact that polygon slices allow each point to be moved means you can directly map a drawn shape onto a real-world shape by dragging each point to its target location on the projection.

Whatever warping you do, the goal is to align the on-screen content with the real-world projection or LED layout. Use a test pattern (Resolume’s Output > Show Test Card is ideal, as it shows a grid and circles) to help align geometry. You would, for example, project the test card and then drag corners and warp points until the grid lines line up with the physical screen’s edges and any distortions are corrected.

Using Resolume’s test card to align overlapping projectors for edge blending. In this image, two projectors overlap by ~15% of their width. The diagonal grid lines and circles help ensure both outputs show the same content in the overlap region
resolume.com
. Before blending, adjust the Input slices so that the test patterns coincide perfectly where the projectors overlap.
resolume.com
resolume.com

Masking on the Output Stage

Just as you can mask slices at the input stage, you can also create Masks in the Output Transformation stage. These output masks let you hide or cut out parts of the final output, which is useful for doing final shaping or for blocking out areas (for example, if part of a projection goes off the screen or you need to avoid a specific zone). Output masks apply after all warping, and they affect the combined output of all slices on a screen.

You add an output mask via the + menu (usually there’s an option like “Add Mask” when in Output stage) or sometimes by right-clicking in the Output canvas. You’ll have the same preset shapes and freeform drawing ability as with slice masks
resolume.com
. Draw the mask shape over the output area – everything outside the mask will be cut off (the default behavior is like an inverse mask to match common usage, meaning content outside the shape is hidden)
resolume.com
. You can invert it if needed to cut a hole instead
resolume.com
. The mask can have as many points as needed, and you can use Bezier curves for smooth shapes
resolume.com
. For example, you could mask a curved top edge of a projection, or mask around stage objects.

One important thing: Masks in Output stage apply to all slices beneath them on that screen
resolume.com
. Essentially, an output mask is a top-layer object in the screen’s stack that affects everything. If you have multiple slices and you add a mask, it will uniformly mask all of them. You can use this to your advantage – e.g., if you have several slices covering a projection surface but the surface has a round shape, one output mask can nicely trim all slices to that round shape.

The mask editing on output works the same: you have Transform and Edit Points modes for the mask, allowing you to move the mask or adjust its points, including adding new points by double-clicking
resolume.com
. Use output masks as the final “cookie cutter” for your projection area.

(Technical note: The Resolume team humorously points out that what they call a mask is actually acting like a crop – because it cuts off outside – but they default it to inverted so it behaves how users expect a mask to work
resolume.com
. The end result is what matters: by default, drawing a mask shape will show content inside that shape and hide content outside.)

Additional Slice Options (Flips, Color, Key, Background)

Each slice in the Output Transformation stage has several per-slice options accessible via the right panel when a slice is selected (or icons near the slice). These include flipping, color adjustments, keying mode, and background toggling:

Flip Horizontally/Vertically: You can mirror a slice’s output by clicking the flip icons (which look like little Pac-Man symbols facing left/right or up/down)
resolume.com
. Horizontal flip will reverse the image left-to-right, and vertical flip top-to-bottom. You can also apply both to rotate 180°. This is useful if, for example, a projector lens mirror causes the image to be reversed, or if you need a mirrored output for an installation. A practical trick: if you have a symmetric setup (left and right screens showing mirror-imaged content), you can create one slice for one side, duplicate it, then use Mirror (via right-click) to flip it across the X axis, and use the flip button to horizontally invert the content
resolume.com
. This way you easily get a mirrored pair without manually remapping the second slice’s input
resolume.com
.

Per-Slice Color Correction: Each slice has its own Brightness, Contrast, and RGB channel adjustments in Output stage (separate from the screen-wide adjustments)
resolume.com
. These controls appear in the slice’s properties on the right. They let you fine-tune the output of that particular slice. For example, if you have two slices going to two projectors and one projector is slightly dimmer, you could increase Brightness on the slice that goes to the dimmer projector
resolume.com
. Or if one slice covers an LED panel that is a different manufacturer with slightly different color balance, you can tweak its RGB levels. You can select multiple slices (even across screens) and adjust their color settings together – the changes will apply to all selected slices uniformly
resolume.com
. This per-slice correction is very helpful in edge blending scenarios too, where you might need to adjust one slice to match the other at the seams
resolume.com
.

“Is Key” (Luminance Key Output): This toggle, labeled Is Key, will make the slice output act as an alpha key signal
resolume.com
. When enabled, the slice’s output is converted to a black-and-white matte: white where the image has any opacity, and black where it’s transparent
resolume.com
. Essentially, it outputs the slice’s alpha channel as a grayscale image. This is useful if you need to send a key/fill pair to a broadcast mixer – for example, one screen could output the fill (the normal graphics) and a second screen outputs the key (mask) by having Is Key enabled
resolume.com
. With a hardware video switcher, you can then overlay graphics with transparency on live video using that key. Most users won’t need this unless working in a production environment with separate keying.

Black BG (Black Background): By default, Resolume outputs slices with transparency where there’s no content (so slices can overlap and you’d see whatever is behind a slice if it has alpha). Enabling Black BG on a slice forces Resolume to render a solid black background under that slice’s content
resolume.com
. In practice, this means the slice will cover up anything behind it on that screen, even if the content has transparent parts
resolume.com
. Use case: if you have two slices overlapping on one output and you never want the lower slice to show through the upper one’s empty areas, turn on Black BG for the top slice
resolume.com
. This way, the top slice is always composited as an opaque piece (with black acting as “empty” to the viewer). A fun description by Resolume: “Black BG is not a racially themed 70s cover band,” but a tool to ensure you don’t accidentally reveal content underneath when a slice’s content goes transparent
resolume.com
. Essentially, it fills the slice’s bounding box with black whenever there’s no image, rather than leaving it empty.

Edge Blending and Projector Overlap (Soft Edge Tools)

When using multiple projectors to create one large continuous image, you’ll often have their outputs overlap slightly to seamlessly blend between them. Edge blending is the technique of gradually feathering out the brightness at the edges of each projector’s image so that where they overlap, the combined light isn’t double-bright and the transition is smooth
resolume.com
resolume.com
. Resolume Arena includes dedicated soft-edge blending features in the Advanced Output to handle this scenario.

Here’s a step-by-step on using the edge blending tools:

Overlap in Input Stage: First, set up your slices so that they overlap in the Input Selection stage in the same proportion as your projectors overlap on the screen
resolume.com
. For example, if you plan to overlap projectors by 20% of their width, then your slices should overlap by 20% in the composition. This typically means the slices’ rectangles in Input Selection will extend past the halfway point (for two projectors scenario) such that there’s a region of the comp that is covered by both slices. A minimum of ~15% overlap is recommended for a good blend zone
resolume.com
. You can use Resolume’s test card to verify the overlap: display the test pattern and adjust slice positions until the overlapping area on the physical projection shows identical content from both projectors
resolume.com
 (e.g., the diagonal lines of the test card should line up exactly when seen through both projectors).

Align in Output Stage: Next, in Output Transformation, align each projector’s slice to the projection surface edges. Use Perspective warping (the corner points) on each slice to perfectly line up the grids or test pattern on the screen
resolume.com
. Essentially, you treat each projector’s output separately and make sure it is squared up and aligned without worrying about the blend yet. Adjust the corners so that both projector grids match up and there’s no keystone or scaling error. The idea is to get the two images overlapping congruently – they should be spatially aligned and the only remaining issue is that the overlap area is double-bright.

Enable Edge Blend: Now, for each slice that overlaps, turn on Edge Blending. In the Advanced Output interface, each slice has a Soft Edge or Blend toggle (exact UI may vary: it could be a checkbox labeled “Blend” or an icon next to the slice). Activate it for the slices that share an overlap. The moment you enable it, Resolume will automatically apply a default blending gradient on that slice’s overlapping edge
resolume.com
. In our two-projector example, you’d enable it on both overlapping slices. Resolume will dim the intensity of each slice’s image across the overlap region, so that at the very center of the overlap, each projector is at ~50% brightness, summing to 100% total – thus achieving an even blend
resolume.com
. The overlap should immediately look more uniform with no hard seam.

Fine-Tune Blending: After turning it on, you have access to soft edge fine-tuning parameters for each blended slice. These typically include:

Gamma (per Color Channel) – often labeled as Gamma R, Gamma G, Gamma B: This adjusts the overall brightness curve of the blend for each color channel
resolume.com
. By tweaking these, you can correct any color tint or brightness mismatch in the overlapping zone. For instance, if the overlap looks a bit greenish, you might reduce the Gamma G for the blend area
resolume.com
.

Power – this controls the steepness of the blend curve
resolume.com
. A higher power makes the falloff steeper toward the middle of the overlap (i.e., darker edges with a quicker transition to full brightness), while a lower power makes a more gradual blend. Adjusting this helps hide the edges depending on projector brightness and overlap width.

Luminance – this adjusts the midpoint brightness of the blend
resolume.com
. Essentially it can raise or lower the brightness at the center of the overlap region. Use this if the overlap looks too dim or too bright relative to the rest of the image.

Overall Gamma – sometimes a general Gamma for the whole blend area (in addition to per-channel) to control overall blend brightness
resolume.com
.

These parameters let you perfect the blend so that the transition is invisible. You might display a full-white image and adjust so that you can’t see a brighter band where they overlap, or display a grayscale gradient and ensure it looks uniform. Each projector and setup may need slightly different tuning.

Black Level Compensation: One tricky issue with projectors is black isn’t truly black – when they overlap, “black” (actually dark gray) on top of “black” becomes a lighter gray. The Black Level Compensation feature helps with this by slightly brightening the non-overlapped parts to match the elevated black in the overlapped parts
resolume.com
resolume.com
. In other words, since overlapping two projectors makes the overlapped “black” twice as bright as a single projector’s “black”, Resolume lets you raise the black output of the rest of the image to avoid a visible halo. Black Level Compensation is found on the Output tab (screen settings) and when enabled, you can dial it in so that the black background looks consistent across the whole screen
resolume.com
resolume.com
. Technically, it makes the normally un-blended areas a bit brighter (lifting their black) to meet the overlap area
resolume.com
resolume.com
. You only need this if you notice the overlap region’s dark parts look different from non-overlap dark parts. It’s a subtle adjustment to perfect the seamlessness.

After these steps, the two (or more) projectors should act like one continuous seamless display. The edges will be softly feathered together with no visible split, and brightness/color should be consistent.

Final output after applying soft-edge blending on two overlapping projectors. The content flows across both projectors with no visible seam in the middle, creating one seamless image (the soft edge region is in the center, perfectly blended).
resolume.com
resolume.com

Recap/Example: Suppose you have a wide panoramic projection using two projectors with a 1920×1080 feed each, overlapping by 20% (about 384 pixels) in the middle. You set up Screen 1 and Screen 2, each 1920×1080, and in Input Selection you create Slice 1 covering 0–1152px of the comp width (which is 1920+384 overlap = 2304 total comp width for example) and Slice 2 covering 768–1920px (so 384px overlap with slice 1). In Output stage, you align Slice 1 to projector 1, Slice 2 to projector 2, get the grids aligned. Turn on Edge Blend for both slices – the overlapping 384px region now is soft-faded. You fine-tune gamma and power until any slight bright line disappears. The result: one giant image ~ in our example comp 2304px wide ~ projected flawlessly by two projectors acting as one.

DMX Lighting Output (Pixel Mapping via Lumiverses & Fixtures)

Beyond screens and projectors, Resolume Arena’s Advanced Output also includes tools for DMX pixel mapping – essentially outputting your visuals to LED strips, matrices, or lighting fixtures by driving them with DMX lighting protocols. This feature is exclusive to Arena and allows you to treat groups of lights as another kind of “output” for your content
resolume.com
.

The key concepts here are Lumiverses and Fixtures:

DMX Lumiverse: This is Resolume’s virtual representation of one or more DMX universes
resolume.com
. (A DMX universe is 512 channels, typically controlling up to 170 RGB LEDs or other parameters.) A Lumiverse is like a container for fixtures that also corresponds to an actual output destination (like an Art-Net node or USB DMX device). You can add a Lumiverse in the Advanced Output (using the + menu, “Add Lumiverse”)
resolume.com
. It will appear in the left panel just like screens do, but with a different icon. You can create multiple lumiverses if you need more than one universe or have multiple outputs/nodes.

Each Lumiverse has similar controls to a screen: it has Opacity/Brightness/Contrast/RGB adjustments (so you can dim or color-correct the entire set of fixtures together)
resolume.com
resolume.com
. Additionally, lumiverses have toggles like Auto-Span and Align Output:

Auto-Span: When on (default), if you add fixtures that collectively require more than 512 channels, Resolume will automatically extend the lumiverse to multiple sequential DMX universes
resolume.com
. This saves you from manually splitting fixtures across universes. For example, if you map 600 channels worth of pixels, Auto-Span will create a second universe for the overflow and manage addressing behind the scenes
resolume.com
.

Align Output: This ensures that fixtures don’t get split across the boundary of two DMX universes in a weird way (some devices require complete pixels in one universe)
resolume.com
. With Align Output on, if a pixel’s channels would straddle the 512 boundary, Resolume will instead move the whole pixel to the next universe, leaving a few unused channels at the end of the previous one to keep things tidy
resolume.com
. Generally, leave this on unless you have a reason to disable it.

Once you have a lumiverse, you will later need to assign it to an actual DMX output (like Art-Net or sACN network interface, or an Enttec USB DMX, etc.). In the Advanced Output UI, you can right-click the lumiverse (or use controls on right) to set the DMX Output settings: choose the protocol (Art-Net, sACN, KiNet, etc.), the network interface (if multiple NICs), and the Universe/subnet numbers to output to
resolume.com
resolume.com
. (This essentially patches your virtual lumiverse to real lighting hardware.)

Fixtures: Inside a lumiverse, you add Fixtures, which represent individual lights or LED segments. When you create a lumiverse, Resolume by default adds one generic fixture of 1 RGB pixel
resolume.com
. You will typically change this to match your actual fixture. From the fixture dropdown on the right, select the appropriate fixture profile – Resolume comes with many common fixture profiles (e.g. “LED strip 50 pixels”, or a specific moving head’s channels)
resolume.com
. If your exact fixture is not listed, you can create a custom profile using the Fixture Editor utility
resolume.com
 (accessible via the Support or a button, it lets you define how many channels, what they do, etc. for a new fixture type).

Suppose you have an LED strip with 60 RGB LEDs. You would select a fixture preset that has 60 pixels (RGB each). Once selected, you’ll see that fixture represented on the Input Selection canvas as a long rectangle subdivided into 60 segments (each segment corresponding to one pixel)
resolume.com
. You can then position and scale this fixture slice over your composition in Input Selection, just like any other slice
resolume.com
! Essentially, you are deciding which part of the comp each LED will sample its color from. For example, you might size the fixture slice to 60×1 pixels and lay it at the bottom of your composition if you want the strip to mirror the bottom row of your video content. Or if the LEDs are arranged vertically on stage, you might make the fixture slice 1×60 and place it accordingly. In the example given in the docs, they made an 800×50 pixel area of the comp map to a 16-pixel LED tube, positioning it at the bottom-center of the comp so that it picks up content from that region
resolume.com
resolume.com
. Resolume will sample the colors from the comp under each pixel’s small square (usually sampling the center of each cell by default)
resolume.com
.

You can rotate and move the fixture slice to match the orientation of how the lights are physically laid out
resolume.com
. For instance, if your LED strip runs diagonally on your stage set, you can rotate the fixture slice outline diagonally across your comp’s preview so the content will scroll along it correctly. This mapping is entirely up to your creative design – you might even overlay multiple fixture slices on the same comp area for artistic effects (though usually each fixture is mapped to a unique area of content).

If you have multiple identical fixtures, you can add more via the + menu (Add Fixture) or simply duplicate the one you configured. When you add a new fixture, Resolume will, by default, use the same type as the last one you configured
resolume.com
, to save time if you’re adding many of the same. You then position the new fixture’s slice area to correspond to where that fixture should sample content. For example, if you had three LED tubes forming a triangle shape on stage, you’d add three fixtures in the lumiverse, each representing one tube. You could position their slices on the comp in a triangular arrangement as well (perhaps corresponding to a triangular section of your content)
resolume.com
. The documentation example describes doing exactly this: three LED bars arranged in a triangle, each fixture slice rotated and placed to cover a triangular layout on the composition
resolume.com
.

While positioning fixture slices, you can zoom and pan the Input Selection canvas for precision (scroll wheel or CTRL +/– to zoom, spacebar drag to pan)
resolume.com
. This is helpful when aligning many small pixel fixtures.

After setting up all the fixture slices on the Input side (defining what part of the comp each light will display), there is no Output Transformation for lumiverses in the way there is for screens – the “output” for a lumiverse is simply the DMX channel data. However, you do need to ensure your lumiverse is patched to the correct output:

Sending to Lights: Go to the Lumiverse’s settings (in Advanced Output) and choose the output method. If using Art-Net or sACN (network), select the network card (if more than one) and set the Universe numbers. For example, Lumiverse 1 could be Art-Net on Net 0, Subnet 0, Universe 0. If Auto-Span has created Universe 1 as well, that will automatically go to Subnet 0, Universe 1, etc., unless configured otherwise
resolume.com
. If using a DMX USB interface, ensure Resolume recognizes it and select it (note: some devices might not be supported or need drivers). Once configured, Resolume will output the DMX signals when the composition is running, driving the fixtures’ colors. You should see your LED strips or lights now responding to the video content mapped under those fixture slices.

This essentially turns Resolume into a low-res media server for lighting – your video can play across LED strips and panels in sync with your main screens. The integration means you can coordinate lights and video easily
resolume.com
resolume.com
.

When to use: Use the DMX output mapping when you have LED decor, pixel tubes, or other lighting elements that you want to run as part of your visual content. It’s especially common in stage designs where LED strips outline shapes or in immersive installations. Resolume’s approach (Lumiverse/fixtures) is very much like other pixel mapping software, but built right into your VJ workflow.

Lastly, if things get very complex: Slice Routing with Screens to Slices can even route a whole screen (or virtual screen) into a slice of another screen
resolume.com
. This is advanced usage but for completeness: you could, for example, output a finished blended projection from two virtual screens, then take that combined result as an input to a third screen’s slice to further warp or map it
resolume.com
. This kind of “screen-ception” allows multi-pass mapping (first do edge blending on simple rectangles, then map the blended output onto a 3D object). Such techniques are rarely needed unless doing very elaborate shows, but know that Advanced Output can handle it with Virtual Outputs and routing screens into slices as inputs
resolume.com
.

Output Monitoring and Previewing

When you’ve set up an advanced output with multiple screens or odd mappings, it’s important to be able to preview what’s going where. Resolume provides a few ways to monitor the outputs:

Output Monitor Panel: In Resolume’s interface, there is an “Output Monitor” (the small preview window that by default shows your main output). By default, this shows the composition’s output before advanced mapping. However, as of Resolume 7.7 and above, you can configure preview monitors to show Advanced Output screens as well
resolume.com
resolume.com
. By clicking the little radar icon on a monitor and selecting a screen, you can create a preview panel for any screen from the Advanced Output
resolume.com
. For example, you could have one preview showing Screen 1 and another showing Screen 2. This is incredibly useful to verify that, say, your “LED wall content” screen is correct while your main projector content is different. It essentially gives you a real-time look at each final mapped output on your control interface (without having to physically look at the LED or projector).

Direct Advanced Output Window: Another approach is to open the Advanced Output window itself on a secondary monitor (maybe your laptop’s HDMI out if you’re not using it for actual output) and observe the Output Transformation canvas. Some operators move the Advanced Output window to a spare monitor and maximize it, effectively using it as an output preview monitor
resolume.com
resolume.com
. In the past, Resolume didn’t have the built-in preview for screens, so this was a common workaround. If you do this, you can even hide the left and right panels (Resolume doesn’t currently have a one-click “preview mode” for this, but as a trick, one could imagine minimizing those panels)
resolume.com
. However, with the new preview monitors feature, you likely won’t need to do this hack.

Test Card and Identify: Use Output > Show Test Card to verify mappings at any time
resolume.com
. This test pattern shows a labeled grid, which is useful for confirming that each slice is outputting the correct area (the pattern includes a moving element so you can see orientation too). Also Output > Identify Displays will flash numbers/color on each actual output screen
resolume.com
 – handy if you’re not sure which screen in Resolume corresponds to which physical projector/monitor (the OS sometimes enumerates displays oddly)
resolume.com
. The identify overlay shows the OS display number and some info so you can match them up.

Monitoring Performance: If you’re pushing a lot of outputs, keep an eye on Output > Show FPS which will display the frame rate on the output monitor
resolume.com
. If your frame rate drops too low (consistently below ~30), you may need to optimize content or reduce resolution. Ideally, frame rate matches your output refresh (60fps for 60Hz, etc.)
resolume.com
.

Snapshot: A handy tool under Output menu is Snapshot, which takes a screenshot of the current output and saves it (and imports into your deck)
resolume.com
. This is not exactly an Advanced Output feature, but if you want to capture what your mapping looks like at a given moment (for documentation or troubleshooting), Snapshot will grab the composition output as seen after all transformations.

In Resolume 7.13+, you can create multiple preview panels and even drag them around or resize in the interface. So you could, for example, set up one panel to preview the composition, another to preview Screen 1 (Projectors), and another for Screen 2 (LED walls). This gives you situational awareness of all outputs in your control UI. Just remember these previews are just on your control machine’s GUI – they don’t affect the actual output.

Finally, practice safe mapping: before the show, double-check your mappings by sending test patterns or known calibration content. This ensures that all slices are correctly positioned and nothing is spilling over incorrectly. Use the Advanced Output’s flexibility to make last-minute tweaks if the projector moved or an LED panel was repositioned – often you can just nudge some warping points or slice positions and quickly adapt to the real-world setup.

Conclusion and Further Tips

We’ve covered every major component of Resolume Arena’s Advanced Output: from setting up screens and routing inputs, through slicing the composition, masking and warping outputs, to blending edges between projectors and even outputting to DMX lighting. Here are a few final tips and reminders:

Keyboard Shortcuts: Remember useful shortcuts in Advanced Output: holding CTRL while dragging to disable snapping
resolume.com
, Shift+arrow keys to nudge 10px, spacebar (in DMX fixture editing) to pan the view
resolume.com
, and CTRL+SHIFT+D to disable outputs in emergencies
resolume.com
. These can speed up your mapping workflow. Right-clicking slices and screens also reveals many useful actions (duplicate, split slice, reset transform, etc.) – don’t forget those context menus.

Organization: Name your screens and slices descriptively (e.g., “LeftProj”, “Center LED”, “DJ Booth LED”) by double-clicking to rename. This helps avoid confusion especially in complex setups.

Presets & Backups: Make use of the preset system to save different configurations. It’s a lifesaver if you tour between venues. Also, manually back up your preset XML files or note their location (Documents/Resolume Arena/presets/screensetup)
resolume.com
 so you don’t lose your mapping work.

Test thoroughly: If using edge blends, project a gray image and walk across the blend to catch any visible lines. If using DMX outputs, test that all LEDs respond correctly and in order (the Fixture Editor’s test or a simple gradient video can help verify mapping). And if using layer routing, double-check that your composition doesn’t inadvertently still display those layers (use the bypass/solo toggles appropriately).

By mastering the Advanced Output, you unlock Resolume Arena’s full potential as a media server for live shows. Whether you’re projection mapping a building, running a multi-screen club visual setup, or integrating lights with video, the Advanced Output panel is your control center. Take time to get comfortable with slicing and warping – it’s both a technical and artistic tool. With practice, you’ll be able to set up complex mappings quickly and adapt to on-site changes with ease, giving your audience a flawlessly mapped visual experience. Happy mapping!