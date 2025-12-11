# Bluetooth setup
- sudo apt install bluez*
- sudo apt install rfkill --fix-broken
- sudo systemctl enable bluetooth
- sudo rfkill list
- sudo hciconfig -a
- sudo rfkill unblock bluetooth
- sudo hciconfig hci0 up
- sudo bluetoothctl
  - > scan on
  - > pair <MAC_ADDRESS>
  
# gpio
- sudo raspi-config # interface options enable SPI
- sudo usermod -a -G spi $USER
