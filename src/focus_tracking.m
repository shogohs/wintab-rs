#import <AppKit/AppKit.h>
#import <ApplicationServices/ApplicationServices.h>
#import <dispatch/dispatch.h>
#import <unistd.h>

static NSMutableDictionary<NSNumber *, id> *wintab_ax_observers;
static NSMutableArray *wintab_workspace_observers;
static void *wintab_space_context;

extern void wintab_record_focus(int pid, const void *window);
extern void wintab_resident_space_changed(void *context);

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

void wintab_focus_tracking_start(void *context) {
    if (wintab_ax_observers != nil) return;
    wintab_space_context = context;
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
    id space_change = [center addObserverForName:NSWorkspaceActiveSpaceDidChangeNotification
                                          object:workspace queue:NSOperationQueue.mainQueue
                                      usingBlock:^(NSNotification *note) {
        if (wintab_space_context != NULL) wintab_resident_space_changed(wintab_space_context);
    }];
    wintab_workspace_observers = [NSMutableArray arrayWithObjects:activation, launch, terminate, space_change, nil];
}

void wintab_focus_tracking_stop(void) {
    NSNotificationCenter *center = [NSWorkspace sharedWorkspace].notificationCenter;
    for (id observer in wintab_workspace_observers) [center removeObserver:observer];
    wintab_workspace_observers = nil;
    wintab_ax_observers = nil;
    wintab_space_context = NULL;
}
