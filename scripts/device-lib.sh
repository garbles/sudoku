# Loads the values of the tablet model DEVICE (default: pro) from devices/$DEVICE.conf. The images of a model go to
# images/$DEVICE/ (build.sh). The scripts calculate all screen positions from these values.
DEVICE=${DEVICE:-pro}; export DEVICE   # the Python scripts read it also
# SCRIPTS_DIR is the folder of the scripts. A caller that changed the folder after its start sets it.
SCRIPTS_DIR=${SCRIPTS_DIR:-$(cd "$(dirname "$0")" && pwd)}
DEVICE_CONF="$SCRIPTS_DIR/devices/$DEVICE.conf"
[ -f "$DEVICE_CONF" ] || { echo "unknown device: $DEVICE (no $DEVICE_CONF)" >&2; exit 2; }
. "$DEVICE_CONF"
# The page of the manual and the analytics in layout units (portrait): the screen over PAGE_SCALE, rounded.
PAGE_W=$(awk -v w=$PHYS_W -v s=$PAGE_SCALE 'BEGIN{ printf "%d", w/s + 0.5 }')
PAGE_H=$(awk -v h=$PHYS_H -v s=$PAGE_SCALE 'BEGIN{ printf "%d", h/s + 0.5 }')
