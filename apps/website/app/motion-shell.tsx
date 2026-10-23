"use client";

import { useEffect } from "react";
import { gsap } from "gsap";
import Lenis from "lenis";

export default function MotionShell() {
  useEffect(() => {
    const media = gsap.matchMedia();
    media.add("(prefers-reduced-motion: no-preference)", () => {
      const lenis = new Lenis({ autoRaf: true, smoothWheel: true });
      gsap.fromTo("[data-reveal]", { y: 18, opacity: 0 }, {
        y: 0, opacity: 1, duration: 0.72, stagger: 0.09, ease: "power3.out",
      });
      return () => lenis.destroy();
    });
    return () => media.revert();
  }, []);
  return null;
}
