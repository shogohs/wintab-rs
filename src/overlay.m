#import <AppKit/AppKit.h>
#import <QuartzCore/QuartzCore.h>

static NSPanel *wintab_panel;
static NSMutableArray<NSView *> *wintab_rows;
static NSMutableArray<NSString *> *wintab_titles;
static NSTextField *wintab_selected_title;
static void (*wintab_mouse_action)(NSInteger, int, void *);
static void *wintab_mouse_context;

@interface WintabWindowItem : NSView
@property(nonatomic) NSInteger index;
@property(nonatomic, strong) NSImage *icon;
@end

@implementation WintabWindowItem
- (void)updateTrackingAreas {
    for (NSTrackingArea *area in self.trackingAreas) [self removeTrackingArea:area];
    NSTrackingArea *area = [[NSTrackingArea alloc] initWithRect:NSZeroRect
                                                        options:NSTrackingMouseEnteredAndExited | NSTrackingMouseMoved | NSTrackingActiveAlways | NSTrackingInVisibleRect
                                                          owner:self userInfo:nil];
    [self addTrackingArea:area];
    [super updateTrackingAreas];
}
- (void)drawRect:(NSRect)dirtyRect {
    [super drawRect:dirtyRect];
    [self.icon drawInRect:NSInsetRect(self.bounds, 4, 4)];
}
- (void)mouseEntered:(NSEvent *)event {
    if (wintab_mouse_action != NULL) wintab_mouse_action(self.index, 0, wintab_mouse_context);
}
- (void)mouseMoved:(NSEvent *)event {
    if (wintab_mouse_action != NULL) wintab_mouse_action(self.index, 0, wintab_mouse_context);
}
- (BOOL)acceptsFirstMouse:(NSEvent *)event { return YES; }
@end

int wintab_overlay_handle_click(void) {
    if (wintab_panel == nil) return -1;
    NSPoint screen_point = [NSEvent mouseLocation];
    if (!NSPointInRect(screen_point, wintab_panel.frame)) return -1;
    NSPoint window_point = [wintab_panel convertPointFromScreen:screen_point];
    NSView *content = wintab_panel.contentView;
    NSPoint content_point = [content convertPoint:window_point fromView:nil];
    for (WintabWindowItem *row in wintab_rows) {
        NSPoint row_point = [row convertPoint:content_point fromView:content];
        if (NSPointInRect(row_point, row.bounds)) {
            if (wintab_mouse_action != NULL) wintab_mouse_action(row.index, 1, wintab_mouse_context);
            return 1;
        }
    }
    return 0;
}

static void set_selected_row(NSInteger selected) {
    for (NSUInteger index = 0; index < wintab_rows.count; index++) {
        NSView *row = wintab_rows[index];
        BOOL is_selected = (NSInteger)index == selected;
        row.layer.backgroundColor = is_selected ? NSColor.selectedContentBackgroundColor.CGColor : NSColor.clearColor.CGColor;
    }
    if (selected >= 0 && (NSUInteger)selected < wintab_rows.count) {
        wintab_selected_title.stringValue = wintab_titles[(NSUInteger)selected];
    }
}

int wintab_overlay_show(const char *const *labels, const int *pids, size_t count, NSInteger selected,
                        void (*action)(NSInteger, int, void *), void *context) {
    @autoreleasepool {
        if (count == 0) return 0;
        [NSApplication sharedApplication];
        wintab_mouse_action = action;
        wintab_mouse_context = context;
        const CGFloat icon_size = 94.0;
        const CGFloat item_spacing = 12.0;
        NSRect screen = NSScreen.screens.firstObject.visibleFrame;
        CGFloat max_width = screen.size.width - 32.0;
        CGFloat width = MIN(MAX(360.0, (CGFloat)count * (icon_size + 20.0) + 40.0), max_width);
        CGFloat fitted_icon_size = MAX(1.0, MIN(icon_size, (width - 40.0 - (CGFloat)(count - 1) * item_spacing) / (CGFloat)count));
        CGFloat item_stride = fitted_icon_size + item_spacing;
        CGFloat row_size = fitted_icon_size + 8.0;
        CGFloat height = 186.0;
        NSRect frame = NSMakeRect(NSMidX(screen) - width / 2.0,
                                  NSMidY(screen) - height / 2.0,
                                  width,
                                  height);
        wintab_panel = [[NSPanel alloc] initWithContentRect:frame
                                                   styleMask:NSWindowStyleMaskBorderless | NSWindowStyleMaskNonactivatingPanel
                                                     backing:NSBackingStoreBuffered
                                                       defer:NO];
        if (wintab_panel == nil) return 0;
        wintab_panel.level = NSStatusWindowLevel;
        wintab_panel.acceptsMouseMovedEvents = YES;
        wintab_panel.collectionBehavior = NSWindowCollectionBehaviorCanJoinAllSpaces | NSWindowCollectionBehaviorFullScreenAuxiliary;
        wintab_panel.opaque = NO;
        NSColor *fallback_background = [NSColor.windowBackgroundColor colorWithAlphaComponent:0.96];
        wintab_panel.backgroundColor = NSColor.clearColor;
        wintab_panel.hasShadow = YES;

        NSView *content = [[NSView alloc] initWithFrame:NSMakeRect(0, 0, width, height)];
        content.wantsLayer = YES;
        wintab_rows = [NSMutableArray arrayWithCapacity:count];
        wintab_titles = [NSMutableArray arrayWithCapacity:count];
        NSMutableDictionary<NSNumber *, NSImage *> *icons = [NSMutableDictionary dictionary];
        for (NSUInteger index = 0; index < count; index++) {
            CGFloat x = (width - ((CGFloat)count * item_stride - item_spacing)) / 2.0 + (CGFloat)index * item_stride;
            WintabWindowItem *row = [[WintabWindowItem alloc] initWithFrame:NSMakeRect(x, 58.0, row_size, row_size)];
            row.wantsLayer = YES;
            row.layer.cornerRadius = 12.0;
            row.index = (NSInteger)index;
            NSNumber *pid = @(pids[index]);
            NSImage *icon = icons[pid];
            if (icon == nil) {
                icon = [NSRunningApplication runningApplicationWithProcessIdentifier:pids[index]].icon;
                if (icon != nil) icons[pid] = icon;
            }
            if (icon == nil) icon = [NSImage imageNamed:NSImageNameApplicationIcon];
            row.icon = icon;
            NSString *title = [NSString stringWithUTF8String:labels[index]] ?: @"";
            [wintab_titles addObject:title.length == 0 ? @"タイトルなし" : title];
            [content addSubview:row];
            [wintab_rows addObject:row];
        }
        wintab_selected_title = [NSTextField labelWithString:@""];
        wintab_selected_title.frame = NSMakeRect(24, 16, width - 48, 24);
        wintab_selected_title.alignment = NSTextAlignmentCenter;
        wintab_selected_title.lineBreakMode = NSLineBreakByTruncatingMiddle;
        wintab_selected_title.font = [NSFont systemFontOfSize:13.0];
        [content addSubview:wintab_selected_title];
        if (@available(macOS 26.0, *)) {
            NSGlassEffectView *glass = [NSGlassEffectView new];
            glass.frame = content.frame;
            glass.cornerRadius = 16.0;
            glass.style = NSGlassEffectViewStyleRegular;
            glass.contentView = content;
            wintab_panel.contentView = glass;
        } else {
            content.layer.cornerRadius = 16.0;
            content.layer.backgroundColor = fallback_background.CGColor;
            wintab_panel.contentView = content;
        }
        set_selected_row(selected);
        [wintab_panel orderFrontRegardless];
        [wintab_panel displayIfNeeded];
        return 1;
    }
}

void wintab_overlay_select(NSInteger selected) {
    @autoreleasepool {
        if (wintab_panel != nil) set_selected_row(selected);
    }
}

void wintab_overlay_hide(void) {
    @autoreleasepool {
        [wintab_panel orderOut:nil];
        wintab_panel = nil;
        wintab_rows = nil;
        wintab_titles = nil;
        wintab_selected_title = nil;
        wintab_mouse_action = NULL;
        wintab_mouse_context = NULL;
    }
}
