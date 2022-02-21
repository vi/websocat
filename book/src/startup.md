# Websocat startup and further flow

1. Websocat is started. User provides command-line options.
2. Websocat's registry scans for all possible long CLI options.
3. Executable crate builds a StrTree, specifying a Websocat session. Alternatively, StrTree is parsed from user CLI parameter.
4. StrTree, along with the list of free-standing CLI options, gets reified into a tree of Nodes.
5. Lints are run to check if there any problems with user input.
6. Root Node gets started (run). Directly or indirectly, it is actually a Session node.
7. Session run (simultaneously) two subnodes: left and right. As a result, bytestream or datagram sources and sinks are obtained both from left and right subnodes. Running the left subnode may recur back into Session, allowing it to handle multiple simultaneous connections.
8. Session interconnects obtained sources and sinks into up to two Directions for copying data from left to right and vice versa.
9. Session runs both Directions until no more data gets available.
10. After all directions are finished, Session waits for other spawned Sessions to finish (if needed) and exist. This exists Websocat.
