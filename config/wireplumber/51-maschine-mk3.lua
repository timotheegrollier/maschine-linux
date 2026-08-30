-- WirePlumber <=0.4 — fix MK3 channel map (alternative au .conf)
-- Placer dans ~/.config/wireplumber/main.lua.d/51-maschine-mk3.lua
-- Copié depuis autotel/maschine-mk3-linux (MIT)

rule = {
  matches = {
    {
      { "device.name", "matches", "alsa_card.usb-Native_Instruments_Maschine_MK3_*" },
    },
  },
  apply_properties = {
    ["api.alsa.use-acp"] = false,
    ["api.acp.auto-profile"] = false,
    ["api.acp.auto-port"] = false,
    ["device.profile"] = "pro-audio",
  },
}

table.insert(alsa_monitor.rules, rule)
