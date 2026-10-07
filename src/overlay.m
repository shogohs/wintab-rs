#import <AppKit/AppKit.h>
#import <QuartzCore/QuartzCore.h>
#import <dispatch/dispatch.h>
#import <ApplicationServices/ApplicationServices.h>
#import <unistd.h>

static NSPanel *wintab_panel;
static NSScrollView *wintab_scroll;
static NSMutableArray<NSView *> *wintab_rows;
static NSMutableArray<NSString *> *wintab_titles;
static NSTextField *wintab_selected_title;
static NSStatusItem *wintab_status_item;
static int (*wintab_menu_action)(int, void *);
static void *wintab_menu_context;
static NSMutableDictionary<NSNumber *, id> *wintab_ax_observers;
static NSMutableArray *wintab_workspace_observers;

extern void wintab_record_focus(int pid, const void *window);

@interface WintabAXObserver : NSObject
{
@public
    AXObserverRef observer;
    AXUIElementRef application;
    CFRunLoopSourceRef source;
    pid_t pid;
}
@end

@implementation WintabAXObserver
- (void)dealloc {
    if (source != NULL) {
        CFRunLoopRemoveSource(CFRunLoopGetMain(), source, kCFRunLoopDefaultMode);
    }
    if (self->observer != NULL) CFRelease(self->observer);
    if (application != NULL) CFRelease(application);
}
@end

static void record_application_focus(pid_t pid, AXUIElementRef app) {
    if (app == NULL) return;
    AXUIElementSetMessagingTimeout(app, 0.2);
    CFTypeRef window = NULL;
    if (AXUIElementCopyAttributeValue(app, kAXFocusedWindowAttribute, &window) == kAXErrorSuccess && window != NULL) {
        wintab_record_focus(pid, window);
        CFRelease(window);
    }
}

static void focused_window_changed(AXObserverRef observer, AXUIElementRef element,
                                   CFStringRef notification, void *refcon) {
    WintabAXObserver *registration = (__bridge WintabAXObserver *)refcon;
    pid_t pid = registration->pid;
    dispatch_async(dispatch_get_main_queue(), ^{
        WintabAXObserver *current = wintab_ax_observers[@(pid)];
        if (current == registration) record_application_focus(pid, current->application);
    });
}

static void observe_application(NSRunningApplication *application) {
    pid_t pid = application.processIdentifier;
    if (pid <= 0 || pid == getpid()) return;
    NSNumber *key = @(pid);
    if (wintab_ax_observers[key] != nil) return;

    WintabAXObserver *registration = [WintabAXObserver new];
    registration->pid = pid;
    registration->application = AXUIElementCreateApplication(pid);
    if (registration->application == NULL) return;
    if (AXObserverCreate(pid, focused_window_changed, &registration->observer) == kAXErrorSuccess) {
        AXError result = AXObserverAddNotification(registration->observer,
                                                   registration->application,
                                                   kAXFocusedWindowChangedNotification,
                                                   (__bridge void *)registration);
        if (result == kAXErrorSuccess) {
            registration->source = AXObserverGetRunLoopSource(registration->observer);
            if (registration->source != NULL) {
                CFRunLoopAddSource(CFRunLoopGetMain(), registration->source, kCFRunLoopDefaultMode);
            }
        }
    }
    wintab_ax_observers[key] = registration;
}

void wintab_focus_tracking_start(void) {
    if (wintab_ax_observers != nil) return;
    wintab_ax_observers = [NSMutableDictionary dictionary];
    NSWorkspace *workspace = [NSWorkspace sharedWorkspace];
    NSNotificationCenter *center = workspace.notificationCenter;
    for (NSRunningApplication *application in workspace.runningApplications) {
        observe_application(application);
    }
    id activation = [center addObserverForName:NSWorkspaceDidActivateApplicationNotification
                                        object:workspace queue:NSOperationQueue.mainQueue
                                    usingBlock:^(NSNotification *note) {
        NSRunningApplication *application = note.userInfo[NSWorkspaceApplicationKey];
        if (application != nil) {
            observe_application(application);
            pid_t pid = application.processIdentifier;
            dispatch_async(dispatch_get_main_queue(), ^{
                WintabAXObserver *registration = wintab_ax_observers[@(pid)];
                if (registration != nil) record_application_focus(pid, registration->application);
            });
        }
    }];
    id launch = [center addObserverForName:NSWorkspaceDidLaunchApplicationNotification
                                    object:workspace queue:NSOperationQueue.mainQueue
                                usingBlock:^(NSNotification *note) {
        observe_application(note.userInfo[NSWorkspaceApplicationKey]);
    }];
    id terminate = [center addObserverForName:NSWorkspaceDidTerminateApplicationNotification
                                       object:workspace queue:NSOperationQueue.mainQueue
                                   usingBlock:^(NSNotification *note) {
        NSRunningApplication *application = note.userInfo[NSWorkspaceApplicationKey];
        if (application != nil) [wintab_ax_observers removeObjectForKey:@(application.processIdentifier)];
    }];
    wintab_workspace_observers = [NSMutableArray arrayWithObjects:activation, launch, terminate, nil];
}

void wintab_focus_tracking_stop(void) {
    NSNotificationCenter *center = [NSWorkspace sharedWorkspace].notificationCenter;
    for (id observer in wintab_workspace_observers) [center removeObserver:observer];
    wintab_workspace_observers = nil;
    wintab_ax_observers = nil;
}

@interface WintabMenuTarget : NSObject
- (void)toggle:(id)sender;
- (void)settings:(id)sender;
- (void)quit:(id)sender;
@end

@implementation WintabMenuTarget
- (void)toggle:(id)sender {
    NSMenuItem *item = (NSMenuItem *)sender;
    BOOL was_enabled = item.state == NSControlStateValueOn;
    int enabled = wintab_menu_action(0, wintab_menu_context);
    if (enabled) wintab_focus_tracking_start();
    else wintab_focus_tracking_stop();
    item.state = enabled ? NSControlStateValueOn : NSControlStateValueOff;
    item.title = enabled ? @"一時停止" : @"有効にする";
    wintab_status_item.button.title = enabled ? @"WT" : @"WT⏸";
    if (!was_enabled && !enabled) {
        NSAlert *alert = [NSAlert new];
        alert.messageText = @"wintab-rsを有効にできませんでした";
        alert.informativeText = @"アクセシビリティと入力監視の許可を確認し、許可した後にもう一度お試しください。";
        [alert addButtonWithTitle:@"システム設定を開く"];
        [alert addButtonWithTitle:@"閉じる"];
        if ([alert runModal] == NSAlertFirstButtonReturn) {
            [[NSWorkspace sharedWorkspace] openURL:[NSURL URLWithString:@"x-apple.systempreferences:com.apple.settings.PrivacySecurity"]];
        }
    }
}
- (void)settings:(id)sender {
    wintab_menu_action(1, wintab_menu_context);
}
- (void)quit:(id)sender {
    wintab_menu_action(2, wintab_menu_context);
    [NSApp terminate:nil];
}
@end

static WintabMenuTarget *wintab_menu_target;

int wintab_status_run(int (*action)(int, void *), void *context, int enabled) {
    @autoreleasepool {
        NSApplication *app = [NSApplication sharedApplication];
        app.activationPolicy = NSApplicationActivationPolicyAccessory;
        if (enabled) wintab_focus_tracking_start();
        wintab_menu_action = action;
        wintab_menu_context = context;
        wintab_menu_target = [WintabMenuTarget new];
        wintab_status_item = [[NSStatusBar systemStatusBar] statusItemWithLength:NSVariableStatusItemLength];
        if (wintab_status_item == nil) return 0;
        wintab_status_item.button.title = enabled ? @"WT" : @"WT⏸";
        NSMenu *menu = [NSMenu new];
        NSMenuItem *toggle = [[NSMenuItem alloc] initWithTitle:(enabled ? @"一時停止" : @"有効にする") action:@selector(toggle:) keyEquivalent:@""];
        toggle.target = wintab_menu_target;
        toggle.state = enabled ? NSControlStateValueOn : NSControlStateValueOff;
        [menu addItem:toggle];
        [menu addItem:[NSMenuItem separatorItem]];
        NSMenuItem *settings = [[NSMenuItem alloc] initWithTitle:@"プライバシーとセキュリティ設定" action:@selector(settings:) keyEquivalent:@""];
        settings.target = wintab_menu_target;
        [menu addItem:settings];
        NSMenuItem *quit = [[NSMenuItem alloc] initWithTitle:@"終了" action:@selector(quit:) keyEquivalent:@""];
        quit.target = wintab_menu_target;
        [menu addItem:quit];
        wintab_status_item.menu = menu;
        [app run];
        [[NSStatusBar systemStatusBar] removeStatusItem:wintab_status_item];
        wintab_focus_tracking_stop();
        wintab_status_item = nil;
        wintab_menu_target = nil;
        wintab_menu_action = NULL;
        wintab_menu_context = NULL;
        return 1;
    }
}

void wintab_status_prepare(void) {
    NSApplication *app = [NSApplication sharedApplication];
    app.activationPolicy = NSApplicationActivationPolicyAccessory;
}

void wintab_open_privacy_settings(void) {
    [[NSWorkspace sharedWorkspace] openURL:[NSURL URLWithString:@"x-apple.systempreferences:com.apple.settings.PrivacySecurity"]];
}

void wintab_defer_switch(void (*action)(void *, int), void *context, int reverse) {
    dispatch_async(dispatch_get_main_queue(), ^{
        action(context, reverse);
    });
}

static void set_selected_row(NSInteger selected) {
    for (NSUInteger index = 0; index < wintab_rows.count; index++) {
        NSView *row = wintab_rows[index];
        BOOL is_selected = (NSInteger)index == selected;
        row.layer.backgroundColor = is_selected ? NSColor.selectedContentBackgroundColor.CGColor : NSColor.clearColor.CGColor;
    }
    if (selected >= 0 && (NSUInteger)selected < wintab_rows.count) {
        [wintab_scroll.contentView scrollRectToVisible:wintab_rows[(NSUInteger)selected].frame];
        wintab_selected_title.stringValue = wintab_titles[(NSUInteger)selected];
    }
}

int wintab_overlay_show(const char *const *labels, const int *pids, size_t count, NSInteger selected) {
    @autoreleasepool {
        if (count == 0) return 0;
        [NSApplication sharedApplication];
        CGFloat item_stride = 80.0;
        CGFloat width = MIN(MAX((CGFloat)count * item_stride + 40.0, 360.0), 760.0);
        CGFloat height = 154.0;
        CGFloat viewport_width = width - 40.0;
        CGFloat document_width = MAX((CGFloat)count * item_stride, viewport_width);
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
        content.wantsLayer = YES;
        content.layer.cornerRadius = 16.0;
        content.layer.backgroundColor = wintab_panel.backgroundColor.CGColor;
        wintab_scroll = [[NSScrollView alloc] initWithFrame:NSMakeRect(20, 48, viewport_width, 84)];
        wintab_scroll.hasHorizontalScroller = document_width > viewport_width;
        wintab_scroll.hasVerticalScroller = NO;
        wintab_scroll.autohidesScrollers = YES;
        wintab_scroll.drawsBackground = NO;
        NSView *document = [[NSView alloc] initWithFrame:NSMakeRect(0, 0, document_width, 80)];
        wintab_rows = [NSMutableArray arrayWithCapacity:count];
        wintab_titles = [NSMutableArray arrayWithCapacity:count];
        NSMutableDictionary<NSNumber *, NSImage *> *icons = [NSMutableDictionary dictionary];
        for (NSUInteger index = 0; index < count; index++) {
            NSView *row = [[NSView alloc] initWithFrame:NSMakeRect((CGFloat)index * item_stride + 4.0, 4.0, 72.0, 72.0)];
            row.wantsLayer = YES;
            row.layer.cornerRadius = 12.0;
            NSNumber *pid = @(pids[index]);
            NSImage *icon = icons[pid];
            if (icon == nil) {
                icon = [NSRunningApplication runningApplicationWithProcessIdentifier:pids[index]].icon;
                if (icon != nil) icons[pid] = icon;
            }
            if (icon == nil) icon = [NSImage imageNamed:NSImageNameApplicationIcon];
            if (icon != nil) {
                NSImageView *imageView = [[NSImageView alloc] initWithFrame:NSMakeRect(4, 4, 64, 64)];
                imageView.image = icon;
                imageView.imageScaling = NSImageScaleProportionallyUpOrDown;
                [row addSubview:imageView];
            }
            NSString *title = [NSString stringWithUTF8String:labels[index]] ?: @"";
            [wintab_titles addObject:title.length == 0 ? @"タイトルなし" : title];
            [document addSubview:row];
            [wintab_rows addObject:row];
        }
        wintab_scroll.documentView = document;
        [content addSubview:wintab_scroll];
        wintab_selected_title = [NSTextField labelWithString:@""];
        wintab_selected_title.frame = NSMakeRect(24, 14, width - 48, 24);
        wintab_selected_title.alignment = NSTextAlignmentCenter;
        wintab_selected_title.lineBreakMode = NSLineBreakByTruncatingMiddle;
        wintab_selected_title.font = [NSFont systemFontOfSize:13.0];
        [content addSubview:wintab_selected_title];
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
        wintab_titles = nil;
        wintab_selected_title = nil;
    }
}
