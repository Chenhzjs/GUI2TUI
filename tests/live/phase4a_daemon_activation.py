"""Install an isolated activation switch; never edit host/system services."""
import os
import pathlib
import shlex

root = pathlib.Path(os.environ["RESULT_DIR"])
services = root / "data" / "dbus-1" / "services"
services.mkdir(parents=True)
launcher = pathlib.Path("/usr/libexec/at-spi-bus-launcher")
assert launcher.is_file(), "inspect this distribution's actual AT-SPI launcher path"
wrapper = root / "launch-accessibility"
wrapper.write_text(
    "#!/bin/sh\n"
    f"test ! -e {shlex.quote(str(root / 'activation-blocked'))} || exit 75\n"
    f"exec {shlex.quote(str(launcher))}\n"
)
wrapper.chmod(0o700)
(services / "org.a11y.Bus.service").write_text(
    f"[D-BUS Service]\nName=org.a11y.Bus\nExec={wrapper}\n"
)
