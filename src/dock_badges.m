#import <AppKit/AppKit.h>
#import <ApplicationServices/ApplicationServices.h>

static CFTypeRef copy_attribute(AXUIElementRef element, CFStringRef name, NSTimeInterval deadline) {
    NSTimeInterval remaining = deadline - NSProcessInfo.processInfo.systemUptime;
    if (remaining <= 0) return NULL;
    if (AXUIElementSetMessagingTimeout(element, (float)MIN(0.03, remaining)) != kAXErrorSuccess) return NULL;
    CFTypeRef value = NULL;
    if (AXUIElementCopyAttributeValue(element, name, &value) != kAXErrorSuccess) return NULL;
    return value;
}

static NSString *bundle_key(CFTypeRef value) {
    NSURL *url = NULL;
    if (value != NULL && CFGetTypeID(value) == CFURLGetTypeID()) {
        url = (__bridge NSURL *)value;
    } else if (value != NULL && CFGetTypeID(value) == CFStringGetTypeID()) {
        NSString *string = (__bridge NSString *)value;
        url = [string hasPrefix:@"/"] ? [NSURL fileURLWithPath:string] : [NSURL URLWithString:string];
    }
    return url.isFileURL ? url.URLByStandardizingPath.path : nil;
}

NSDictionary<NSString *, NSString *> *wintab_read_dock_badges(void) {
    @autoreleasepool {
        // ponytail: one 150ms best-effort Dock snapshot; unreadable or hidden items may be omitted, and AXStatusLabel is undocumented by design.
        NSMutableDictionary *badges = [NSMutableDictionary dictionary];
        NSTimeInterval deadline = NSProcessInfo.processInfo.systemUptime + 0.15;
        NSRunningApplication *dock = nil;
        for (NSRunningApplication *app in NSWorkspace.sharedWorkspace.runningApplications) {
            if ([app.bundleIdentifier isEqualToString:@"com.apple.dock"]) { dock = app; break; }
        }
        if (dock == nil) return badges;
        AXUIElementRef app = AXUIElementCreateApplication(dock.processIdentifier);
        if (app == NULL) return badges;
        CFArrayRef children = copy_attribute(app, kAXChildrenAttribute, deadline);
        CFRelease(app);
        if (children == NULL || CFGetTypeID(children) != CFArrayGetTypeID()) {
            if (children != NULL) CFRelease(children);
            return badges;
        }
        for (id child in (__bridge NSArray *)children) {
            if (deadline <= NSProcessInfo.processInfo.systemUptime) break;
            if (CFGetTypeID((__bridge CFTypeRef)child) != AXUIElementGetTypeID()) continue;
            AXUIElementRef node = (__bridge AXUIElementRef)child;
            CFTypeRef role = copy_attribute(node, kAXRoleAttribute, deadline);
            BOOL is_list = role != NULL && CFGetTypeID(role) == CFStringGetTypeID() && CFEqual(role, kAXListRole);
            if (role != NULL) CFRelease(role);
            if (!is_list) continue;
            CFTypeRef list_children = copy_attribute(node, kAXChildrenAttribute, deadline);
            if (list_children == NULL) continue;
            if (CFGetTypeID(list_children) != CFArrayGetTypeID()) { CFRelease(list_children); continue; }
            NSArray *items = CFBridgingRelease(list_children);
            for (id item in items) {
                if (deadline <= NSProcessInfo.processInfo.systemUptime) break;
                if (CFGetTypeID((__bridge CFTypeRef)item) != AXUIElementGetTypeID()) continue;
                AXUIElementRef dock_item = (__bridge AXUIElementRef)item;
                CFTypeRef item_subrole = copy_attribute(dock_item, kAXSubroleAttribute, deadline);
                BOOL is_application_item = item_subrole != NULL && CFGetTypeID(item_subrole) == CFStringGetTypeID() && CFEqual(item_subrole, kAXApplicationDockItemSubrole);
                if (item_subrole != NULL) CFRelease(item_subrole);
                if (!is_application_item) continue;
                CFTypeRef running = copy_attribute(dock_item, kAXIsApplicationRunningAttribute, deadline);
                BOOL is_running = running != NULL && CFGetTypeID(running) == CFBooleanGetTypeID() && CFBooleanGetValue(running);
                if (running != NULL) CFRelease(running);
                if (!is_running) continue;
                CFTypeRef url = copy_attribute(dock_item, kAXURLAttribute, deadline);
                CFTypeRef label = copy_attribute(dock_item, CFSTR("AXStatusLabel"), deadline);
                NSString *key = url == NULL ? nil : bundle_key(url);
                NSString *text = label != NULL && CFGetTypeID(label) == CFStringGetTypeID() ? (__bridge NSString *)label : nil;
                if (key != nil && text != nil) {
                    text = [text stringByTrimmingCharactersInSet:NSCharacterSet.whitespaceAndNewlineCharacterSet];
                    if (text.length > 0 && ![text isEqualToString:@"0"]) badges[key] = text;
                }
                if (url != NULL) CFRelease(url);
                if (label != NULL) CFRelease(label);
            }
        }
        CFRelease(children);
        return badges;
    }
}
