# waybar-timer

This script implements a **simple** and **customizable** timer for your bar.

- specify a command to execute when the timer expires (e.g. notify-send, shell script, ...)
- interactive:
  * e.g. scroll to increase / decrease timer
  * click to start predefined timers
  * while changing a timer a notification displays when the timer will expire
  * pause timer

![screenshot set timer](screenshots/setTimer.gif) (set a timer)

![screenshot cancel timer](screenshots/cancelTimer.gif) (cancel a timer)

![screenshot set predefined timer](screenshots/predefinedTimer.gif) (start predefined timer)

![screenshot set predefined timer 2 and increase it](screenshots/predefinedTimer2.gif) (start other predefined timer and increase it)

![screenshot see expiry time](screenshots/expiryTimePreview.gif) (watch expiry time when you change a timer)

Even though the repo is named [`waybar-timer`](#), it is a general script and you can use it for every bar.
In particular, if you use [**polybar**](https://github.com/polybar/polybar), then you can find a polybar-specific implementation of this timer [here](https://github.com/jbirnick/polybar-timer).
You can **customize behaviour and appearance in a simple way**.

Use cases: pomodoro timer, self-reminder when next meeting begins, tea/pasta timer, ...

## Dependencies

This script works perfectly **without any dependencies**.

## Installation

1. Download [waybar-timer.sh](https://raw.githubusercontent.com/jbirnick/waybar-timer/master/waybar-timer.sh) from this repo.
2. Make it executable. (`chmod +x waybar-timer.sh`)
3. Copy-paste the [example configuration](#example-configuration) from below into your waybar config and style it.
4. Customize. (see [Customization section](#customization))

## Example Configuration

```json
"custom/timer": {
    "exec": "/path/to/waybar-timer.sh updateandprint",
    "exec-on-event": true,
    "return-type": "json",
    "interval": 5,
    "signal": 4,
    "format": "{icon} {0}",
    "format-icons": {
        "standby": "STANDBY",
        "running": "RUNNING",
        "paused": "PAUSE"
    },
    "on-click": "/path/to/waybar-timer.sh new 25 'notify-send \"Session finished\"'",
    "on-click-middle": "/path/to/waybar-timer.sh cancel",
    "on-click-right": "/path/to/waybar-timer.sh togglepause",
    "on-scroll-up": "/path/to/waybar-timer.sh increase 60 || /path/to/waybar-timer.sh new 1 'notify-send -u critical \"Timer expired.\"'",
    "on-scroll-down": "/path/to/waybar-timer.sh increase -60"
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

The example configuration implements a 25min "pomodoro session" timer with left click, pausing with right click, canceling with middle click, and a normal timer by just scrolling up from the standby mode.

You can customize the different strings, numbers and actions to your own flavor and needs. To understand what the commands do and to implement some different behaviour see the [documentation](#documentation).

If you want to do some really specific stuff and add some functionality, just edit the script. It is really simple. Just take your 10 minutes to understand what it does and then customize it.

## Documentation

Notation: `<...>` are necessary arguments. `[...=DEFAULTVALUE]` are optional arguments,
and if you do not specify them their `DEFAULTVALUE` is used.

The main command of the script is:

- #### `updateandprint`
  This routine will return the current the output (i.e. what you see on the bar) and handle the `ACTION` if the timer expired.
  Namely:
  1. If there is a timer running and its expiry time is <= now, then it executes `ACTION` and kills the timer.
  2. It prints the output info for waybar, in particular the number of remaining minutes.

Now the following commands allow you to control the timer.

- #### `new <MINUTES> [ACTION=""]`
  1. If there is a timer already running this timer gets killed.
  2. Creates a timer of length `MINUTES` minutes and sets its action to `ACTION`. (`ACTION` will be executed once the timer expires.)

- #### `increase <SECONDS>`
  If there is no timer set, nothing happens and it exits with 1.
  If there is a timer set, it is extended by `SECONDS` seconds. `SECONDS` can also be negative, in which case it shortens the timer. Then it exits
  with 0.

- #### `togglepause`
  If there is no timer set at all, it exits with 1. If there is a timer running, the timer gets paused and it exits with 0. If there is a timer set which is already paused, the timer gets resumed and it exits with 0.

- #### `cancel`
  If there is a timer running, the timer gets canceled. The `ACTION` will _not_ be
  executed.

## Tips & Tricks

Note, when there is no timer active, then [`increase`](#increase-seconds) does nothing.
So you might want to use the following command as a replacement for [`increase`](#increase-seconds).
```
waybar-timer.sh increase 60 || waybar-timer.sh new 1 'notify-send "Timer expired."'
```
It increases the existing timer if it's active, and creates a new one minute timer if there is no timer currently running.
So now e.g. scrolling up also does something when there is no timer active - it starts a new timer!

## Known Issues

If you don't (want to) use `dunstify` please see the [dependencies section](#dependencies).
