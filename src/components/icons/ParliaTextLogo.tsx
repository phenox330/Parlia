import React from "react";

const ParliaTextLogo = ({
  width,
  height,
  className,
}: {
  width?: number;
  height?: number;
  className?: string;
}) => {
  const w = width ?? 200;
  const h = height ?? Math.round(w * 0.28);

  return (
    <svg
      width={w}
      height={h}
      viewBox="-15 0 250 56"
      fill="none"
      xmlns="http://www.w3.org/2000/svg"
      className={className}
    >
      <text
        x="50%"
        y="44"
        textAnchor="middle"
        fontFamily="system-ui, -apple-system, sans-serif"
        fontWeight="700"
        fontSize="52"
        letterSpacing="6"
        className="logo-primary"
      >
        PARLIA
      </text>
    </svg>
  );
};

export default ParliaTextLogo;
