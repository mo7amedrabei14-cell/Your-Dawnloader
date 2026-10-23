import type { Metadata } from "next";
import ConnectDeviceClient from "./connect-device-client";

export const metadata: Metadata = {
  title: "Connect a device",
  description: "Approve a Your Dawnloader desktop device to sync download-history metadata to your account.",
};

// Keep this route statically exportable for GitHub Pages. The device code is
// read by the client from the query string after hydration.
export default function ConnectDevicePage() {
  return <ConnectDeviceClient initialCode="" />;
}
