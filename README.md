# waybar-timer

> [!NOTE]  
> This used to be a shell script. Now it is a binary.
> The CLI arguments have changed only slightly but the underlying architecture is completely different.
> Therefore, if you switch from the shell script version to the binary, please make sure to **fully adapt the new default config**.
> In particular, you need to add `hook` for `exec`, remove `interval`, set `exec-on-event` to false, and change `increase -60` to `decrease 60`.
> You also need to start a waybar-timer server _before_ you start waybar.

This script implements a **simple** and **interactive** timer for your bar:
- e.g. scroll to increase / decrease timer
- click to start predefined timers
- while changing a timer a notification displays when the timer will expire
- pause timer

![screenshot set timer](screenshots/setTimer.gif) (set a timer)

![screenshot cancel timer](screenshots/cancelTimer.gif) (cancel a timer)

![screenshot set predefined timer](screenshots/predefinedTimer.gif) (start predefined timer)

![screenshot set predefined timer 2 and increase it](screenshots/predefinedTimer2.gif) (start other predefined timer and increase it)

![screenshot see expiry time](screenshots/expiryTimePreview.gif) (watch expiry time when you change a timer)

Even though the repo is named [`waybar-timer`](#), it is a general script and you can use it for every bar.
In particular, if you use [**polybar**](https://github.com/polybar/polybar), then you can find a polybar-specific implementation of this timer [here](https://github.com/jbirnick/polybar-timer).
You can **customize behaviour and appearance in a simple way**.

Use cases: pomodoro timer, self-reminder when next meeting begins, tea/pasta timer, ...

## Installation

1. Download the binary from the [releases](https://github.com/jbirnick/waybar-timer/releases) (or build it yourself with cargo) and put it in a directory of your choice (e.g. `~/.scripts/`).
2. In the startup script of your compositor, run `/path/to/waybar_timer serve` and make sure it starts **before waybar starts**.
3. Copy-paste the [example configuration](#example-configuration) from below into your waybar config and style it.
4. Customize. (see [Customization section](#customization))

## Example Configuration

```json
"custom/timer": {
    "exec": "/path/to/waybar_timer hook",
    "exec-on-event": false,
    "return-type": "json",
    "format": "{icon} {0}",
    "format-icons": {
        "standby": "STANDBY",
        "running": "RUNNING",
        "paused": "PAUSE"
    },
    "on-click": "/path/to/waybar_timer new 25 'notify-send \"Session finished\"'",
    "on-click-middle": "/path/to/waybar_timer cancel",
    "on-click-right": "/path/to/waybar_timer togglepause",
    "on-scroll-up": "/path/to/waybar_timer increase 60 || /path/to/waybar_timer new 1 'notify-send -u critical \"Timer expired\"'",
    "on-scroll-down": "/path/to/waybar_timer decrease 60"
}
```
The first modification you probably want to make is to replace the `format-icons` by some actually stylish icons.

Furthermore you can style the module using the `timer` class, for example:
```
.timer {
    background-color: #ffee82;
    color: #242424;
    margin: 0 10px;
    padding: 0 10px;
}
```

## Customization

The example configuration implements a 25min "pomodoro session" timer with left click, pausing with right click, canceling with middle click, and an unnamed timer by just scrolling up from the standby mode.

You can customize the different numbers and names to your own flavor and needs. To understand what the commands do and to implement some different behaviour see the [documentation](#documentation).

If you need a specific functionality feel free to open an issue and maybe we can make it happen.

## Documentation

Notation: `<...>` are necessary arguments and `[...]` are optional arguments.

The main commands of the script are :

- #### `serve`
  This is the command you want to put in the startup script of your compositor.
  Make sure you start this server _before_ you start waybar.
  It keeps the state of the timer and provides updates to all the clients who call `hook`.

- #### `hook`
  This is the command which you want to put in your waybar `exec` field.
  It subscribes to the server to get all the updates of the timer.
  Updates are delivered as JSON which is readable by waybar.

Now the following commands allow you to control the timer.

- #### `new <MINUTES> [COMMAND]`
  Creates a new timer of length `MINUTES` minutes.
  If you specify `COMMAND`, it will be executed (within a bash shell) when the timer finishes.

- #### `increase <SECONDS>`
  Extend the current timer by `SECONDS` seconds.

- #### `decrease <SECONDS>`
  Shorten the current timer by `SECONDS` seconds.

- #### `togglepause`
  Pause the current timer.

- #### `cancel`
  Cancel the current timer.
  The specified `COMMAND` will _not_ be executed.

## Tips & Tricks

> [!TIP]
> When there is no timer active, then [`increase`](#increase-seconds) does nothing, i.e. it doesn't change the state of the timer.
> However, you might want it to _start a new timer_.
> You can implement this because `increase` will exit with code 1 when there is no current timer, so you can do:
> ```
> waybar-timer increase 60 || waybar-timer new 1 'notify-send "Timer expired."'
> ```
> Then, if there is an existing timer it gets increased, otherwise a new one minute timer is created.
> This is also implemented in the [example configuration](#example-configuration).
> Just try to scroll up when there is no timer running!

> [!CAUTION]
> Some people use `pkill` to send signals to `waybar`, in order to update some modules.
> But the process name given to `pkill` is matched as a regex, so using `pkill waybar` will _also_ match `waybar_timer` and kill it.
> **So to send signals to waybar, you should use `pkill -x waybar`.**
