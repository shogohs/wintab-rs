#import <AppKit/AppKit.h>
#import <dispatch/dispatch.h>

static NSStatusItem *wintab_status_item;
static NSMenuItem *wintab_accessibility_status;
static NSMenuItem *wintab_input_status;
static NSMenuItem *wintab_toggle_item;
static int (*wintab_menu_action)(int, void *);
static void *wintab_menu_context;

extern void wintab_focus_tracking_start(void *context);
extern void wintab_focus_tracking_stop(void);

@interface WintabMenuTarget : NSObject
- (void)toggle:(id)sender;
- (void)settings:(id)sender;
- (void)refreshPermissions:(id)sender;
- (void)quit:(id)sender;
@end

static void update_permission_status(int status) {
    if (wintab_accessibility_status != nil) {
        wintab_accessibility_status.title = (status & 1) ? @"アクセシビリティ: 許可済み" : @"アクセシビリティ: 未許可";
    }
    if (wintab_input_status != nil) {
        wintab_input_status.title = (status & 2) ? @"入力監視: 許可済み" : @"入力監視: 未許可";
    }
    if (wintab_toggle_item != nil) {
        BOOL enabled = (status & 4) != 0;
        wintab_toggle_item.state = enabled ? NSControlStateValueOn : NSControlStateValueOff;
        wintab_toggle_item.title = @"有効";
        wintab_status_item.button.title = enabled ? @"WT" : @"WT⏸";
    }
}

@implementation WintabMenuTarget
- (void)toggle:(id)sender {
    int enabled = wintab_menu_action(0, wintab_menu_context);
    if (enabled) wintab_focus_tracking_start(wintab_menu_context);
    else wintab_focus_tracking_stop();
    update_permission_status(wintab_menu_action(3, wintab_menu_context));
}
- (void)settings:(id)sender {
    wintab_menu_action(1, wintab_menu_context);
}
- (void)refreshPermissions:(id)sender {
    BOOL was_enabled = wintab_toggle_item.state == NSControlStateValueOn;
    int status = wintab_menu_action(3, wintab_menu_context);
    if (was_enabled && !(status & 4)) wintab_focus_tracking_stop();
    update_permission_status(status);
}
- (void)quit:(id)sender {
    wintab_menu_action(2, wintab_menu_context);
    [NSApp terminate:nil];
}
@end

static WintabMenuTarget *wintab_menu_target;

int wintab_status_run(int (*action)(int, void *), void *context, int enabled,
                      int accessibility, int input_monitoring) {
    @autoreleasepool {
        NSApplication *app = [NSApplication sharedApplication];
        app.activationPolicy = NSApplicationActivationPolicyAccessory;
        wintab_menu_action = action;
        wintab_menu_context = context;
        if (enabled) wintab_focus_tracking_start(context);
        wintab_menu_target = [WintabMenuTarget new];
        wintab_status_item = [[NSStatusBar systemStatusBar] statusItemWithLength:NSVariableStatusItemLength];
        if (wintab_status_item == nil) return 0;
        wintab_status_item.button.title = enabled ? @"WT" : @"WT⏸";
        NSMenu *menu = [NSMenu new];
        wintab_toggle_item = [[NSMenuItem alloc] initWithTitle:@"有効" action:@selector(toggle:) keyEquivalent:@""];
        wintab_toggle_item.target = wintab_menu_target;
        wintab_toggle_item.state = enabled ? NSControlStateValueOn : NSControlStateValueOff;
        [menu addItem:wintab_toggle_item];
        [menu addItem:[NSMenuItem separatorItem]];
        wintab_accessibility_status = [[NSMenuItem alloc] initWithTitle:(accessibility ? @"アクセシビリティ: 許可済み" : @"アクセシビリティ: 未許可") action:nil keyEquivalent:@""];
        wintab_accessibility_status.enabled = NO;
        [menu addItem:wintab_accessibility_status];
        wintab_input_status = [[NSMenuItem alloc] initWithTitle:(input_monitoring ? @"入力監視: 許可済み" : @"入力監視: 未許可") action:nil keyEquivalent:@""];
        wintab_input_status.enabled = NO;
        [menu addItem:wintab_input_status];
        NSMenuItem *refresh = [[NSMenuItem alloc] initWithTitle:@"権限状態を再確認" action:@selector(refreshPermissions:) keyEquivalent:@""];
        refresh.target = wintab_menu_target;
        [menu addItem:refresh];
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
        wintab_accessibility_status = nil;
        wintab_input_status = nil;
        wintab_toggle_item = nil;
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
