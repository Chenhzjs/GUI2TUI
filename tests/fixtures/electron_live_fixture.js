const { app, BrowserWindow, Menu } = require("electron");
const path = require("path");

app.commandLine.appendSwitch("force-renderer-accessibility", "complete");

app.whenReady().then(() => {
  const window = new BrowserWindow({
    width: 640,
    height: 520,
    title: "GUI2TUI Electron Fixture",
  });
  Menu.setApplicationMenu(
    Menu.buildFromTemplate([
      {
        label: "Tools",
        submenu: [
          {
            label: "Activate Demo",
            click: () => window.webContents.executeJavaScript("activateDemo()"),
          },
        ],
      },
    ]),
  );
  window.loadFile(path.join(__dirname, "electron_live_fixture.html"));
});

app.on("window-all-closed", () => app.quit());

