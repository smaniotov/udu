---
type: research
status: closed
blocked_by: [04]
---

# What device-access policy is acceptable on Wayland Linux?

## Evidence
| Fact | Verified at |
|---|---|
| The current upstream README instructs users to join the `input` group and warns against running as root. | `/home/smaniotov/Documents/external/wayvibes/README.md:47-68` |
| The research brief found logind/udev ACL (`uaccess`) as a safer least-privilege alternative where available, but did not establish uniform behavior across distributions. | External research brief citing Linux input and libevdev documentation |

## Conclusion
The first release will document direct `/dev/input` access as a prerequisite and recommend the existing upstream `input` group setup where the user's distribution does not provide a usable session ACL. It will never require root and will report permission failures with an actionable message. Distribution-specific `logind`/udev ACL support remains a deployment enhancement rather than a universal assumption.

## Consequence
Installation and troubleshooting documentation must explain the permission prerequisite, warn against root execution, and distinguish missing permissions from an unavailable or misidentified keyboard device.
