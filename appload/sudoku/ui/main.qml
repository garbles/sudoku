// The QML frontend of Sudoku (REQUIREMENTS.md section 14.4, INVARIANTS.md section 18).
//
// AppLoad loads this file from resources.rcc when the user opens the app, and it starts
// backend/entry, which starts the program `sudoku`. The frontend does only these things:
//
// - It shows the qtfb framebuffer of the program on the full window (FBController).
// - It puts the UI mode on the full window and the pen mode on the pen area (DisplayMethodArea).
//   The program gives the pen area in a message on the AppLoad socket.
// - It gives the rotation of the tablet to the framebuffer item, and it shows the top bar of
//   AppLoad after a drag from the top edge.
// - It stops the program when AppLoad unloads the frontend.
//
// The program draws all screens and gets the pen and touch input from the framebuffer item.
// src/launch.rs has the same IDs, message types and pen area calculation: a test compares them.
//
// AppLoad 0.5.1 has DisplayMethodArea, but its FBController has no fbRotation, no fillMode and no
// framebufferSize, and its window has no globalRotation (they came with AppLoad 0.6.0). This file
// uses these names only when they are there: a fixed use of a name that is not there stops the
// load of the file.
import QtQuick 2.5
import net.asivery.AppLoad 1.0
import net.asivery.Framebuffer 1.0
import net.asivery.ApploadUtils

Item {
    id: root
    anchors.fill: parent

    // AppLoad connects to this signal of each frontend. This frontend does not send it: the close
    // button of the app stops the program, and AppLoad then closes the window.
    signal close

    // LCH-5: the app ID of manifest.json and the framebuffer ID of the program. The corpus app
    // gets its own values (appload/make-folder.sh).
    readonly property string appId: "sudoku"
    readonly property int framebufferId: 0x5355444F
    // The message of the program with the pen area: "x y width height" in the coordinates of the
    // window, or the text of noArea.
    readonly property int penAreaMessage: 1
    readonly property string noArea: "none"
    // AppLoad gives the values of DisplayMethodArea to the tablet by their position. The names
    // Content (3) and UI (4) of the item are in the opposite order, thus use the numbers.
    readonly property int penMode: 0
    readonly property int uiMode: 3

    // The window of AppLoad: the first parent that has the rotation value or the top bar
    // function. They are internal names of AppLoad. null: not found.
    property var win: null
    // LCH-8: the frontend turns the picture only when it found the rotation value and the top bar
    // function, and the framebuffer item can turn. Else the app stays in portrait.
    readonly property bool canTurn: root.win !== null && root.win.globalRotation !== undefined
        && typeof root.win.swipeFromTheTop === "function" && fb.fbRotation !== undefined
    // The pen area that the program gave last: [x, y, width, height]. null: no area.
    property var area: null
    // True after AppLoad unloaded the frontend: the program stops.
    property bool stopped: false

    // True if the framebuffer item gives the size of the framebuffer (AppLoad 0.6.0).
    readonly property bool sized: fb.framebufferSize !== undefined
    // LCH-3: the scale s and the offset (ox, oy) of the framebuffer picture in the window. The
    // picture is the framebuffer as the player sees it: a rotation of 90 degrees exchanges its
    // width and its height. It is the largest picture that fits, at the center. null: not known.
    // An AppLoad whose framebuffer item gives no size draws the framebuffer on the full item, with
    // no rotation: on the full screen, one pixel of the framebuffer is one pixel of the window.
    readonly property var picture: {
        if (!fb.active)
            return null
        if (!root.sized)
            return { s: 1, ox: 0, oy: 0 }
        const size = fb.framebufferSize
        if (!(size.width > 0) || !(size.height > 0) || !(fb.width > 0) || !(fb.height > 0))
            return null
        const turned = fb.fbRotation === FBController.Deg90L || fb.fbRotation === FBController.Deg90R
        const w = turned ? size.height : size.width
        const h = turned ? size.width : size.height
        const s = Math.min(fb.width / w, fb.height / h)
        return { s: s, ox: (fb.width - s * w) / 2, oy: (fb.height - s * h) / 2 }
    }
    // LCH-4: the window is the full screen. Without the window of AppLoad, the full screen is
    // known only from the picture: one pixel of the framebuffer is one pixel of the window.
    readonly property bool fullScreen: root.win !== null
        ? root.win.fullscreen === true
        : root.sized && root.picture !== null && root.picture.s === 1 && root.picture.ox === 0 && root.picture.oy === 0
    readonly property bool minimized: root.win !== null && root.win.minimized === true
    // LCH-4: no pen area when the program gave no area, when the program stopped, when the window
    // is minimized or not active, and in a window that is not the full screen.
    readonly property bool penAreaOn: root.area !== null && !root.stopped && fb.active && root.visible
        && !root.minimized && root.fullScreen && root.picture !== null

    // AppLoad calls this function when it unloads the frontend, for example after the X of its
    // top bar. A backend stays in the background unless the frontend stops it (LCH-1).
    function unloading() {
        root.stopped = true
        root.area = null
        backend.terminate()
    }

    function findWindow() {
        for (let p = root.parent; p; p = p.parent) {
            if (p.globalRotation !== undefined || typeof p.swipeFromTheTop === "function")
                return p
        }
        return null
    }

    // Gives the rotation of the AppLoad window to the framebuffer item. AppLoad turns the picture
    // and sends the rotation to the program on the qtfb connection. Else the item keeps the
    // rotation 0: the app stays in portrait (LCH-8).
    function turn() {
        if (root.canTurn)
            fb.fbRotation = root.win.globalRotation
    }

    // The pen area in a message of the program. null: no area, or the text is not correct.
    function readArea(text) {
        if (text === root.noArea)
            return null
        const v = text.trim().split(" ").map(Number)
        return v.length === 4 && v.every(n => isFinite(n)) && v[2] > 0 && v[3] > 0 ? v : null
    }

    Component.onCompleted: {
        root.win = root.findWindow()
        if (!root.canTurn)
            console.log("sudoku: no rotation value or no top bar function in the AppLoad window: the app stays in portrait")
        // The picture keeps its shape in each window: the app never shows a cut screen.
        if (fb.fillMode !== undefined)
            fb.fillMode = FBController.PreserveAspectFit
        root.turn()
        // The program connects to the framebuffer when this endpoint connects. Thus the
        // framebuffer item has the rotation of the tablet before the first frame.
        backend.applicationID = root.appId
    }

    Connections {
        target: root.win
        ignoreUnknownSignals: true
        function onGlobalRotationChanged() {
            root.turn()
        }
    }

    AppLoad {
        id: backend
        onMessageReceived: (type, contents) => {
            if (type === root.penAreaMessage)
                root.area = root.readArea(contents)
        }
    }

    FBController {
        id: fb
        anchors.fill: parent
        framebufferID: root.framebufferId
        allowScaling: true

        // A drag from the top edge shows the top bar of AppLoad.
        onDragDown: {
            if (root.win !== null && typeof root.win.swipeFromTheTop === "function")
                root.win.swipeFromTheTop()
        }

        // The full window always has the UI mode (the colors refresh fully).
        DisplayMethodArea {
            anchors.fill: parent
            displayMethod: root.uiMode
        }

        // The pen area: the cell of the ink gets the fast pen waveform.
        DisplayMethodArea {
            visible: root.penAreaOn
            x: root.penAreaOn ? root.picture.ox + root.picture.s * root.area[0] : 0
            y: root.penAreaOn ? root.picture.oy + root.picture.s * root.area[1] : 0
            width: root.penAreaOn ? root.picture.s * root.area[2] : 0
            height: root.penAreaOn ? root.picture.s * root.area[3] : 0
            displayMethod: root.penMode
        }
    }
}
