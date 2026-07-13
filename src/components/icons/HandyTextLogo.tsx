/* eslint-disable i18next/no-literal-string -- renders only the brand wordmark, never translated */
import React from "react";

// ChadVoice wordmark. Keeps the original component name and viewBox so every
// call site renders at the same proportions as the old Handy wordmark.
const HandyTextLogo = ({
  width,
  height,
  className,
}: {
  width?: number;
  height?: number;
  className?: string;
}) => {
  return (
    <svg
      width={width}
      height={height}
      className={className}
      viewBox="0 0 930 328"
      fill="none"
      xmlns="http://www.w3.org/2000/svg"
    >
      <text
        x="465"
        y="164"
        textAnchor="middle"
        dominantBaseline="central"
        className="logo-primary"
        style={{
          font: "800 172px system-ui, -apple-system, 'Segoe UI', sans-serif",
          letterSpacing: "-0.02em",
          paintOrder: "stroke",
          stroke: "var(--color-logo-stroke)",
          strokeWidth: 10,
          strokeLinejoin: "round",
        }}
      >
        ChadVoice
      </text>
    </svg>
  );
};

export default HandyTextLogo;
