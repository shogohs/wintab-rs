#import <AppKit/AppKit.h>
#import <QuartzCore/QuartzCore.h>

static NSPanel *wintab_panel;
static NSScrollView *wintab_scroll;
static NSMutableArray<NSView *> *wintab_rows;

static void set_selected_row(NSInteger selected) {
    for (NSUInteger index = 0; index < wintab_rows.count; index++) {
        NSView *row = wintab_rows[index];
        BOOL is_selected = (NSInteger)index == selected;
        row.layer.backgroundColor = is_selected ? NSColor.selectedContentBackgroundColor.CGColor : NSColor.clearColor.CGColor;
        for (NSView *child in row.subviews) {
            if ([child isKindOfClass:NSTextField.class]) {
                NSTextField *label = (NSTextField *)child;
                label.textColor = is_selected ? NSColor.selectedMenuItemTextColor : NSColor.labelColor;
            }
        }
    }
    if (selected >= 0 && (NSUInteger)selected < wintab_rows.count) {
        [wintab_scroll.contentView scrollRectToVisible:wintab_rows[(NSUInteger)selected].frame];
    }
}

int wintab_overlay_show(const char *const *labels, size_t count, NSInteger selected) {
    @autoreleasepool {
        if (count == 0) return 0;
        [NSApplication sharedApplication];
        CGFloat row_height = 30.0;
        CGFloat width = 640.0;
        CGFloat visible_height = MIN((CGFloat)count * row_height, 600.0);
        CGFloat height = visible_height + 24.0;
        NSRect screen = NSScreen.mainScreen.visibleFrame;
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
        wintab_panel.collectionBehavior = NSWindowCollectionBehaviorCanJoinAllSpaces | NSWindowCollectionBehaviorFullScreenAuxiliary;
        wintab_panel.opaque = NO;
        wintab_panel.backgroundColor = [NSColor.windowBackgroundColor colorWithAlphaComponent:0.96];
        wintab_panel.hasShadow = YES;

        NSView *content = [[NSView alloc] initWithFrame:NSMakeRect(0, 0, width, height)];
        wintab_scroll = [[NSScrollView alloc] initWithFrame:NSMakeRect(12, 12, width - 24, visible_height)];
        wintab_scroll.hasVerticalScroller = count * row_height > visible_height;
        wintab_scroll.drawsBackground = NO;
        NSView *document = [[NSView alloc] initWithFrame:NSMakeRect(0, 0, width - 24, (CGFloat)count * row_height)];
        wintab_rows = [NSMutableArray arrayWithCapacity:count];
        for (NSUInteger index = 0; index < count; index++) {
            NSView *row = [[NSView alloc] initWithFrame:NSMakeRect(0,
                                                                 (CGFloat)(count - index - 1) * row_height,
                                                                 width - 24,
                                                                 row_height)];
            row.wantsLayer = YES;
            NSTextField *label = [NSTextField labelWithString:[NSString stringWithUTF8String:labels[index]] ?: @""];
            label.frame = NSMakeRect(12, 4, width - 48, row_height - 8);
            label.lineBreakMode = NSLineBreakByTruncatingTail;
            label.font = [NSFont systemFontOfSize:14.0];
            [row addSubview:label];
            [document addSubview:row];
            [wintab_rows addObject:row];
        }
        wintab_scroll.documentView = document;
        [content addSubview:wintab_scroll];
        wintab_panel.contentView = content;
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
        wintab_scroll = nil;
        wintab_rows = nil;
    }
}
