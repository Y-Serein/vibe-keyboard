import { useRef, useEffect } from "react";
import { listen } from "@tauri-apps/api/event";

interface ScreenProps {
  mode: "standby" | "normal" | "select" | "allow" | "notify";
}

/**
 * LCD Screen — renders daemon's framebuffer via Canvas putImageData.
 * Falls back to text-based display if no frame data arrives.
 */
function Screen(props: ScreenProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    const unlisten = listen<{ width: number; height: number; pixels_b64: string }>(
      "frame-update",
      (event) => {
        const canvas = canvasRef.current;
        if (!canvas) return;
        const ctx = canvas.getContext("2d");
        if (!ctx) return;

        const { width, height, pixels_b64 } = event.payload;
        if (canvas.width !== width) canvas.width = width;
        if (canvas.height !== height) canvas.height = height;

        // Decode base64 to RGB565 bytes
        const binary = atob(pixels_b64);
        const rgb565 = new Uint8Array(binary.length);
        for (let i = 0; i < binary.length; i++) {
          rgb565[i] = binary.charCodeAt(i);
        }

        // Convert RGB565 to RGBA ImageData
        const imageData = ctx.createImageData(width, height);
        const pixelCount = width * height;
        for (let i = 0; i < pixelCount; i++) {
          const lo = rgb565[i * 2];
          const hi = rgb565[i * 2 + 1];
          const pixel = lo | (hi << 8);
          const r = ((pixel >> 11) & 0x1f) << 3;
          const g = ((pixel >> 5) & 0x3f) << 2;
          const b = (pixel & 0x1f) << 3;
          imageData.data[i * 4] = r;
          imageData.data[i * 4 + 1] = g;
          imageData.data[i * 4 + 2] = b;
          imageData.data[i * 4 + 3] = 255;
        }

        ctx.putImageData(imageData, 0, 0);
      },
    );

    return () => {
      unlisten.then((f) => f());
    };
  }, []);

  return (
    <div style={{
      background: "#111827",
      borderRadius: "10px",
      marginBottom: "14px",
      overflow: "hidden",
      border: props.mode === "allow" ? "2px solid #34d399" : "2px solid #1f2937",
    }}>
      <canvas
        ref={canvasRef}
        width={800}
        height={340}
        style={{
          width: "100%",
          display: "block",
          imageRendering: "pixelated",
          borderRadius: "8px",
        }}
      />
    </div>
  );
}

export default Screen;
