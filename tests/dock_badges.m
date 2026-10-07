#import <Foundation/Foundation.h>
#import <assert.h>
#import "../src/dock_badges.m"

int main(void) {
    @autoreleasepool {
        NSURL *bundle_url = [NSURL fileURLWithPath:@"/Applications/Test.app"];
        assert([bundle_key((__bridge CFTypeRef)bundle_url) isEqualToString:@"/Applications/Test.app"]);

        CFStringRef file_url_string = CFSTR("file:///Applications/Test.app");
        assert([bundle_key(file_url_string) isEqualToString:@"/Applications/Test.app"]);
        assert([bundle_key(CFSTR("/Applications/Other/../Test.app")) isEqualToString:@"/Applications/Test.app"]);
        assert(bundle_key(CFSTR("https://example.com/Test.app")) == nil);
        assert(bundle_key(kCFBooleanTrue) == nil);
        assert(bundle_key(NULL) == nil);
    }
    return 0;
}
