#import <AppKit/AppKit.h>
#import <QuartzCore/QuartzCore.h>
#import <CoreFoundation/CoreFoundation.h>
#import <stdatomic.h>

extern NSDictionary<NSString *, NSString *> *wintab_read_dock_badges(void);

static NSPanel *wintab_panel;
static NSMutableArray<NSView *> *wintab_rows;
static NSMutableArray<NSString *> *wintab_titles;
static NSTextField *wintab_selected_title;
static void (*wintab_mouse_action)(NSInteger, int, void *);
static void *wintab_mouse_context;
static atomic_flag wintab_badge_worker = ATOMIC_FLAG_INIT;
static NSUInteger wintab_panel_generation;

@interface WintabWindowItem : NSView
@property(nonatomic) NSInteger index;
@property(nonatomic, strong) NSImage *icon;
@property(nonatomic, copy) NSString *badge;
@property(nonatomic, copy) NSString *bundleKey;
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
    NSRect iconRect = NSInsetRect(self.bounds, 4, 4);
    [self.icon drawInRect:iconRect];
    if (self.badge.length == 0 || iconRect.size.width < 24) return;
    CGFloat height = iconRect.size.height * 0.23;
    CGFloat maxWidth = iconRect.size.width * 0.62;
    CGFloat fontSize = MAX(10.0, height * 0.62);
    NSFont *font = [NSFont boldSystemFontOfSize:fontSize];
    NSString *text = self.badge;
    NSMutableParagraphStyle *paragraph = [NSMutableParagraphStyle new];
    paragraph.alignment = NSTextAlignmentCenter;
    paragraph.lineBreakMode = NSLineBreakByClipping;
    NSDictionary *attributes = @{ NSFontAttributeName: font, NSForegroundColorAttributeName: NSColor.whiteColor, NSParagraphStyleAttributeName: paragraph };
    CGFloat horizontalPadding = height * 0.375;
    CGFloat verticalPadding = height * 0.075;
    NSSize textSize = [text sizeWithAttributes:attributes];
    BOOL fits = textSize.width + horizontalPadding * 2.0 <= maxWidth && textSize.height + verticalPadding * 2.0 <= height;
    CGFloat width = MIN(maxWidth, textSize.width + horizontalPadding * 2.0);
    NSRect badgeRect = NSMakeRect(NSMaxX(iconRect) - width, NSMaxY(iconRect) - height, width, height);
    [[NSColor colorWithRed:0.90 green:0.12 blue:0.16 alpha:1.0] setFill];
    if (fits) {
        [[NSBezierPath bezierPathWithRoundedRect:badgeRect xRadius:height / 2.0 yRadius:height / 2.0] fill];
        [NSGraphicsContext saveGraphicsState];
        NSRectClip(badgeRect);
        NSRect textRect = NSMakeRect(NSMinX(badgeRect) + (badgeRect.size.width - textSize.width) / 2.0,
                                     NSMinY(badgeRect) + (badgeRect.size.height - textSize.height) / 2.0,
                                     textSize.width,
                                     textSize.height);
        [text drawAtPoint:textRect.origin withAttributes:attributes];
        [NSGraphicsContext restoreGraphicsState];
    } else {
        CGFloat diameter = MIN(height * 0.45, MIN(iconRect.size.width, iconRect.size.height));
        NSRect dotRect = NSMakeRect(NSMaxX(iconRect) - diameter, NSMaxY(iconRect) - diameter, diameter, diameter);
        [[NSBezierPath bezierPathWithOvalInRect:dotRect] fill];
    }
}

- (void)mouseEntered:(NSEvent *)event {
    if (wintab_mouse_action != NULL) wintab_mouse_action(self.index, 0, wintab_mouse_context);
}
- (void)mouseMoved:(NSEvent *)event {
    if (wintab_mouse_action != NULL) wintab_mouse_action(self.index, 0, wintab_mouse_context);
}
- (BOOL)acceptsFirstMouse:(NSEvent *)event { return YES; }
@end

static void apply_dock_badges(NSDictionary<NSString *, NSString *> *badges, NSUInteger generation) {
    if (wintab_panel == nil || generation != wintab_panel_generation) return;
    for (WintabWindowItem *row in wintab_rows) {
        row.badge = row.bundleKey == nil ? nil : badges[row.bundleKey];
        [row setNeedsDisplay:YES];
    }
    [wintab_panel displayIfNeeded];
}

static void start_dock_badge_read(NSUInteger generation) {
    if (atomic_flag_test_and_set_explicit(&wintab_badge_worker, memory_order_acquire)) return;
    dispatch_async(dispatch_get_global_queue(QOS_CLASS_UTILITY, 0), ^{
        NSDictionary *badges = [wintab_read_dock_badges() copy];
        atomic_flag_clear_explicit(&wintab_badge_worker, memory_order_release);
        CFRunLoopPerformBlock(CFRunLoopGetMain(), kCFRunLoopDefaultMode, ^{
            apply_dock_badges(badges, generation);
        });
        CFRunLoopWakeUp(CFRunLoopGetMain());
    });
}

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
        WintabWindowItem *row = (WintabWindowItem *)wintab_rows[index];
        row.layer.backgroundColor = (NSInteger)index == selected
            ? [NSColor.systemBlueColor colorWithAlphaComponent:0.18].CGColor
            : NSColor.clearColor.CGColor;
    }
    if (selected >= 0 && (NSUInteger)selected < wintab_rows.count) {
        wintab_selected_title.stringValue = wintab_titles[(NSUInteger)selected];
    }
}

int wintab_overlay_show(const char *const *labels, const int *pids, size_t count, NSInteger selected,
                        void (*action)(NSInteger, int, void *), void *context) {
    @autoreleasepool {
        if (count == 0) return 0;
        wintab_panel_generation++;
        [NSApplication sharedApplication];
        wintab_mouse_action = action;
        wintab_mouse_context = context;
        const CGFloat icon_size = 94.0;
        const CGFloat item_spacing = 12.0;
        const CGFloat background_corner_radius = 26.0;
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
        NSMutableDictionary<NSNumber *, NSString *> *bundleKeys = [NSMutableDictionary dictionary];
        for (NSUInteger index = 0; index < count; index++) {
            CGFloat x = (width - ((CGFloat)count * item_stride - item_spacing)) / 2.0 + (CGFloat)index * item_stride;
            WintabWindowItem *row = [[WintabWindowItem alloc] initWithFrame:NSMakeRect(x, 58.0, row_size, row_size)];
            row.wantsLayer = YES;
            row.layer.cornerRadius = row_size * 0.22;
            row.layer.cornerCurve = kCACornerCurveContinuous;
            row.index = (NSInteger)index;
            NSNumber *pid = @(pids[index]);
            NSRunningApplication *runningApp = [NSRunningApplication runningApplicationWithProcessIdentifier:pids[index]];
            NSString *bundleKey = bundleKeys[pid];
            if (bundleKey == nil && runningApp.bundleURL != nil) {
                bundleKey = runningApp.bundleURL.URLByStandardizingPath.path;
                if (bundleKey != nil) bundleKeys[pid] = bundleKey;
            }
            row.bundleKey = bundleKey;
            NSImage *icon = icons[pid];
            if (icon == nil) {
                icon = runningApp.icon;
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
        wintab_selected_title.frame = NSMakeRect(24, 8, width - 48, 38);
        wintab_selected_title.alignment = NSTextAlignmentCenter;
        wintab_selected_title.lineBreakMode = NSLineBreakByTruncatingMiddle;
        wintab_selected_title.font = [NSFont systemFontOfSize:17.0];
        [content addSubview:wintab_selected_title];
        if (@available(macOS 26.0, *)) {
            NSGlassEffectView *glass = [NSGlassEffectView new];
            glass.frame = content.frame;
            glass.cornerRadius = background_corner_radius;
            glass.style = NSGlassEffectViewStyleRegular;
            glass.alphaValue = 1.0;
            glass.wantsLayer = YES;
            glass.layer.cornerRadius = background_corner_radius;
            glass.layer.cornerCurve = kCACornerCurveContinuous;
            glass.layer.masksToBounds = YES;
            NSView *glassContent = [[NSView alloc] initWithFrame:glass.bounds];
            glass.contentView = glassContent;
            [content addSubview:glass positioned:NSWindowBelow relativeTo:nil];
            wintab_panel.contentView = content;
        } else {
            content.layer.cornerRadius = background_corner_radius;
            content.layer.cornerCurve = kCACornerCurveContinuous;
            content.layer.masksToBounds = YES;
            content.layer.backgroundColor = fallback_background.CGColor;
            wintab_panel.contentView = content;
        }
        set_selected_row(selected);
        [wintab_panel orderFrontRegardless];
        [wintab_panel displayIfNeeded];
        start_dock_badge_read(wintab_panel_generation);
        return 1;
    }
}

void wintab_overlay_select(NSInteger selected) {
    @autoreleasepool {
        if (wintab_panel != nil) {
            set_selected_row(selected);
            [wintab_panel displayIfNeeded];
        }
    }
}

void wintab_overlay_hide(void) {
    @autoreleasepool {
        wintab_panel_generation++;
        [wintab_panel orderOut:nil];
        wintab_panel = nil;
        wintab_rows = nil;
        wintab_titles = nil;
        wintab_selected_title = nil;
        wintab_mouse_action = NULL;
        wintab_mouse_context = NULL;
    }
}
