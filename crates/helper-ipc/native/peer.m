#import <Foundation/Foundation.h>
#import <AppKit/AppKit.h>
#import <Security/Security.h>
#include <xpc/xpc.h>
#define MAX_PAYLOAD (64 * 1024)

// Own signing team determines the group and the peer requirement. Source builds
// signed by another developer stay isolated from the production installation.
static NSString *ownTeam(void) {
    SecCodeRef code = NULL;
    CFDictionaryRef info = NULL;
    NSString *team = nil;
    if (SecCodeCopySelf(kSecCSDefaultFlags, &code) == errSecSuccess &&
        SecCodeCopySigningInformation(code, kSecCSSigningInformation, &info) == errSecSuccess) {
        team = [(__bridge NSDictionary *)info objectForKey:(__bridge NSString *)kSecCodeInfoTeamIdentifier];
    }
    if (info) CFRelease(info);
    if (code) CFRelease(code);
    return team;
}

char *lp_helper_service_name(void) {
    @autoreleasepool {
        NSString *team = ownTeam();
        return team.length ? strdup([team stringByAppendingString:@".app.linkpilot.helper"].UTF8String) : NULL;
    }
}

static NSString *requirement(BOOL server) {
    NSString *team = ownTeam();
    if (!team.length) return nil;
    NSString *ids = server ? @"identifier \"app.linkpilot.helper\"" :
        @"(identifier \"app.linkpilot.store\" or identifier \"app.linkpilot.desktop\" or identifier \"app.linkpilot.helper\")";
    return [NSString stringWithFormat:@"anchor apple generic and certificate leaf[subject.OU] = \"%@\" and %@", team, ids];
}

static int enforcePeer(xpc_connection_t connection, NSString *req) {
    if (@available(macOS 12.0, *)) {
        return xpc_connection_set_peer_code_signing_requirement(connection, req.UTF8String);
    }
    return -4;
}

static BOOL rejectedPeer(xpc_object_t reply) {
    if (@available(macOS 12.0, *)) { return reply == XPC_ERROR_PEER_CODE_SIGNING_REQUIREMENT; }
    return YES;
}

// Authenticate a non-sensitive hello on THIS connection before sending any user
// data. XPC peer requirements validate incoming messages, not outgoing payloads.
// https://developer.apple.com/forums/thread/681053
static xpc_object_t exchange(xpc_connection_t connection, xpc_object_t message) {
    dispatch_semaphore_t done = dispatch_semaphore_create(0);
    __block xpc_object_t reply = NULL;
    xpc_connection_send_message_with_reply(connection, message, dispatch_get_global_queue(QOS_CLASS_DEFAULT, 0), ^(xpc_object_t value) {
        reply = value;
        dispatch_semaphore_signal(done);
    });
    if (dispatch_semaphore_wait(done, dispatch_time(DISPATCH_TIME_NOW, 3 * NSEC_PER_SEC))) return NULL;
    return reply;
}

char *lp_helper_request(const unsigned char *bytes, size_t length, size_t *outLength, int *error) {
    @autoreleasepool {
        *error = -1;
        if (!length || length > MAX_PAYLOAD) return NULL;
        char *name = lp_helper_service_name();
        NSString *req = requirement(YES);
        if (!name || !req) { free(name); return NULL; }
        xpc_connection_t connection = xpc_connection_create_mach_service(name, NULL, 0);
        free(name);
        if (enforcePeer(connection, req)) return NULL;
        xpc_connection_set_event_handler(connection, ^(xpc_object_t event) { (void)event; });
        xpc_connection_resume(connection);
        xpc_object_t hello = xpc_dictionary_create(NULL, NULL, 0);
        xpc_dictionary_set_bool(hello, "hello", true);
        xpc_object_t greeted = exchange(connection, hello);
        if (!greeted || xpc_get_type(greeted) != XPC_TYPE_DICTIONARY || !xpc_dictionary_get_bool(greeted, "ready")) {
            *error = rejectedPeer(greeted) ? -3 : -2;
            xpc_connection_cancel(connection);
            return NULL;
        }
        xpc_object_t message = xpc_dictionary_create(NULL, NULL, 0);
        xpc_dictionary_set_data(message, "payload", bytes, length);
        xpc_object_t reply = exchange(connection, message);
        char *result = NULL;
        if (reply && xpc_get_type(reply) == XPC_TYPE_DICTIONARY) {
            size_t size = 0;
            const void *data = xpc_dictionary_get_data(reply, "payload", &size);
            if (data && size && size <= MAX_PAYLOAD) {
                result = malloc(size);
                if (result) { memcpy(result, data, size); *outLength = size; *error = 0; }
            }
        }
        xpc_connection_cancel(connection);
        return result;
    }
}

typedef unsigned char *(*handler_t)(const unsigned char *, size_t, size_t *);
typedef void (*free_t)(unsigned char *, size_t);

int lp_helper_serve(handler_t handler, free_t release) {
    @autoreleasepool {
        char *name = lp_helper_service_name();
        NSString *req = requirement(NO);
        if (!name || !req) { free(name); return -1; }
        xpc_connection_t listener = xpc_connection_create_mach_service(name, NULL, XPC_CONNECTION_MACH_SERVICE_LISTENER);
        free(name);
        xpc_connection_set_event_handler(listener, ^(xpc_object_t object) {
            if (xpc_get_type(object) != XPC_TYPE_CONNECTION) return;
            xpc_connection_t peer = (xpc_connection_t)object;
            if (xpc_connection_get_euid(peer) != geteuid() ||
                enforcePeer(peer, req)) {
                xpc_connection_cancel(peer);
                return;
            }
            xpc_connection_set_event_handler(peer, ^(xpc_object_t message) {
                @autoreleasepool {
                    if (xpc_get_type(message) == XPC_TYPE_ERROR) {
                        // Drop this block's strong peer reference on disconnect.
                        xpc_connection_set_event_handler(peer, ^(xpc_object_t event) { (void)event; });
                        xpc_connection_cancel(peer);
                        return;
                    }
                    if (xpc_get_type(message) != XPC_TYPE_DICTIONARY) return;
                    xpc_object_t reply = xpc_dictionary_create_reply(message);
                    if (!reply) return;
                    if (xpc_dictionary_get_bool(message, "hello")) {
                        xpc_dictionary_set_bool(reply, "ready", true);
                    } else {
                        size_t length = 0, output = 0;
                        const unsigned char *data = xpc_dictionary_get_data(message, "payload", &length);
                        if (!data || !length || length > MAX_PAYLOAD) { xpc_connection_cancel(peer); return; }
                        unsigned char *result = handler(data, length, &output);
                        if (!result || !output || output > MAX_PAYLOAD) {
                            if (result) release(result, output);
                            xpc_connection_cancel(peer); return;
                        }
                        xpc_dictionary_set_data(reply, "payload", result, output);
                        release(result, output);
                    }
                    xpc_connection_send_message(peer, reply);
                }
            });
            xpc_connection_resume(peer);
        });
        xpc_connection_resume(listener);
        dispatch_main();
    }
}

// LaunchServices starts the independently installed app, without inheriting the
// caller's sandbox. Validate the bundle first; the socket authenticates again.
int lp_helper_start(void) {
    @autoreleasepool {
        NSURL *url = [[NSWorkspace sharedWorkspace] URLForApplicationWithBundleIdentifier:@"app.linkpilot.helper"];
        if (!url) return -2;
        SecStaticCodeRef code = NULL;
        SecRequirementRef req = NULL;
        NSString *text = requirement(YES);
        if (!text) return -1;
        OSStatus result = SecStaticCodeCreateWithPath((__bridge CFURLRef)url, kSecCSDefaultFlags, &code);
        if (!result) result = SecRequirementCreateWithString((__bridge CFStringRef)text, kSecCSDefaultFlags, &req);
        if (!result) result = SecStaticCodeCheckValidity(code, kSecCSCheckAllArchitectures | kSecCSStrictValidate, req);
        if (code) CFRelease(code);
        if (req) CFRelease(req);
        if (result) return result;
        NSError *error = nil;
        #pragma clang diagnostic push
        #pragma clang diagnostic ignored "-Wdeprecated-declarations"
        NSRunningApplication *app = [[NSWorkspace sharedWorkspace] launchApplicationAtURL:url
            options:NSWorkspaceLaunchWithoutActivation | NSWorkspaceLaunchAndHide
            configuration:@{} error:&error];
        #pragma clang diagnostic pop
        return app ? 0 : (int)(error.code ?: -1);
    }
}
