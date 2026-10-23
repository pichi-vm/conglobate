# Link-local NAT option: unresolved design investigation

User proposed link-local addressing for the internal NAT link. No production
change or endorsement of a replacement topology is implied by this note.

At 2dc93c5, `NetworkTopology::capture` observes assigned addresses and non-default
routes in all tables. `allocate` selects a private IPv4 /30 and IPv6 /126 that
do not overlap those observations. It cannot discover destination networks
reachable only through a default route. Visible-route collision avoidance is
therefore not a guarantee of all-destination reachability.

Source translation could keep internal addresses off the uplink, but that alone
does not establish correct link-local forwarding behavior. RFC 3927 section 2.7
prohibits router forwarding of IPv4 link-local source or destination addresses:
https://www.rfc-editor.org/rfc/rfc3927.html#section-2.7
Linux behavior under explicit NAT needs separate validation; a successful Linux
experiment would not establish RFC compliance. IPv6 needs its own analysis.

Before adopting this option, use a disposable VM to establish outbound and
return traffic behavior, actual translated uplink addresses, DNS reachability,
reply-only inbound restrictions, route cleanup, and behavior when the supplied
resolver or another reachable endpoint uses link-local addressing. Avoid a
broad connected /16 that shadows unrelated link-local destinations. Even narrow
endpoint routes reserve addresses and cannot guarantee no destination collision.

This investigation remains open. No collision-free or dual-stack claim is made.
