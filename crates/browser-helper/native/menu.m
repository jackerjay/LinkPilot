#import <AppKit/AppKit.h>

@interface LPHelperMenu : NSObject
@property (assign) int (*disableHelper)(void);
- (void)disable:(id)sender;
- (void)permissions:(id)sender;
@end
@implementation LPHelperMenu
- (void)permissions:(id)sender {
    (void)sender;
    [[NSWorkspace sharedWorkspace] openURL:[NSURL URLWithString:@"x-apple.systempreferences:com.apple.preference.security?Privacy_FilesAndFolders"]];
}
- (void)disable:(id)sender {
    (void)sender;
    if (self.disableHelper() == 0) { [NSApp terminate:nil]; }
    else {
        NSAlert *alert = [NSAlert new];
        alert.messageText = @"Cannot disable LinkPilot Helper";
        alert.informativeText = @"The background service could not be stopped. Please retry.";
        [alert runModal];
    }
}
@end

void lp_helper_menu(int (*disable)(void)) {
    @autoreleasepool {
        NSApplication *app = [NSApplication sharedApplication];
        [app setActivationPolicy:NSApplicationActivationPolicyAccessory];
        NSStatusItem *item = [[NSStatusBar systemStatusBar] statusItemWithLength:NSVariableStatusItemLength];
        item.button.title = @"LP+";
        item.button.toolTip = @"LinkPilot Helper";
        NSMenu *menu = [[NSMenu alloc] initWithTitle:@"LinkPilot Helper"];
        NSMenuItem *info = [[NSMenuItem alloc] initWithTitle:@"LinkPilot Helper is running" action:nil keyEquivalent:@""];
        info.enabled = NO;
        [menu addItem:info];
        [menu addItem:[NSMenuItem separatorItem]];
        LPHelperMenu *delegate = [LPHelperMenu new];
        delegate.disableHelper = disable;
        NSMenuItem *permissions = [menu addItemWithTitle:@"Browser Data Permissions…" action:@selector(permissions:) keyEquivalent:@""];
        permissions.target = delegate;
        NSMenuItem *quit = [menu addItemWithTitle:@"Disable LinkPilot Helper" action:@selector(disable:) keyEquivalent:@"q"];
        quit.target = delegate;
        item.menu = menu;
        [app run];
    }
}
