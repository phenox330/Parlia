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
      viewBox="0 0 210 56"
      fill="currentColor"
      xmlns="http://www.w3.org/2000/svg"
      className={className}
    >
      <text
        x="50%"
        y="44"
        textAnchor="middle"
        fontFamily="'Outfit', system-ui, -apple-system, sans-serif"
        fontWeight="700"
        fontSize="52"
        letterSpacing="3"
      >
        PARLIA
      </text>
    </svg>
  );
};

export default ParliaTextLogo;
