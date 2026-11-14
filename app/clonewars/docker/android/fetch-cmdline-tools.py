# Prints "<zip name> <sha1>" of the stable Linux cmdline-tools from Google's
# SDK manifest (repository2-3.xml), for the Dockerfile to download and check.
import sys
import xml.etree.ElementTree as ET

root = ET.parse(sys.argv[1]).getroot()
for pkg in root.iter("remotePackage"):
    if pkg.get("path") != "cmdline-tools;latest":
        continue
    channel = pkg.find("channelRef")
    if channel is None or channel.get("ref") != "channel-0":
        continue
    for archive in pkg.iter("archive"):
        if (archive.findtext("host-os") or "") == "linux":
            complete = archive.find("complete")
            print(complete.findtext("url"), complete.findtext("checksum"))
            sys.exit(0)
sys.exit("no Linux cmdline-tools in Google's manifest")
