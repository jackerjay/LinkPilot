# LinkPilot App Store privacy policy

Effective date: November 9, 2026

LinkPilot routes links to browsers on your Mac. The App Store edition processes
your links and routing rules locally. It has no user account, advertising,
analytics, tracking SDK, or developer-operated server. We do not collect, sell,
or receive your personal data through the app.

## Data on your Mac

Routing rules, preferences, browser choices, and optional local routing
observations stay inside LinkPilot's macOS app container. Recent routing history
is kept in memory. Local routing observations can include a website's hostname,
the source app, selected browser/profile identifier, and time of selection. You
can disable and clear these observations in Settings. LinkPilot and its optional
Helper do not read browsing history, passwords, or cookies.

## Optional browser Helper

Browser-level routing works without a companion application. If you separately
install and enable LinkPilot Helper, it can read Chrome and Brave profile
metadata, such as profile identifiers, display names, and associated email
addresses, to show your available profiles. It reads browser profile metadata
locally and passes the profile list and requested web links between Helper and
the Store app through authenticated local XPC. We do not receive this data.

Helper is distributed separately through the official GitHub releases. Browser
data permissions belong to Helper's own macOS privacy entry. You can revoke
individual browser permissions in System Settings and disable Helper from its
menu. Helper does not change the Store app's sandbox or require Full Disk
Access. Other browsers continue to use browser-level routing in this version.

The app discovers supported installed browsers using macOS system APIs. When you
open a link, macOS passes it to the browser you choose or a routing rule selects.
That browser and the destination website handle the link under their own privacy
policies. Routing a link can therefore share the URL with the selected browser
and website, as it would when opening the link manually.

## Files and network access

Configuration import and export use the macOS file picker. Only files you choose
are read or written for that operation. Exported configuration can contain your
rules and preferences; you control where you save or share it.

The App Store edition does not check GitHub for updates or download updates
outside the App Store. Its network-client entitlement supports the system
WebKit renderer. The app does not send routing data to us.

Choosing the optional Helper download or support links opens GitHub in your
browser. GitHub and your browser handle those requests under their own policies.
The app does not automatically download, install, or update Helper.

## Support and changes

If you choose to contact us through GitHub, GitHub processes the information you
submit under its privacy policy. Do not include private URLs, browsing data, or
credentials in public issues. We may update this policy when the app's behavior
changes. The effective date above identifies this version.

Questions: [LinkPilot support](https://github.com/jackerjay/LinkPilot/issues).
