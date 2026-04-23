interface ParliaMarkProps {
  size?: number | string;
  width?: number | string;
  height?: number | string;
  color?: string;
  className?: string;
}

const ParliaMark = ({
  size,
  width,
  height,
  color,
  className,
}: ParliaMarkProps) => {
  const w = width ?? size ?? 24;
  const h = height ?? size ?? 24;
  const c = color ?? "currentColor";
  return (
    <svg
      width={w}
      height={h}
      viewBox="0 0 60 60"
      fill="none"
      xmlns="http://www.w3.org/2000/svg"
      className={className}
    >
      <rect
        x="18.6"
        y="12.6"
        width="22.8"
        height="34.8"
        rx="11.4"
        stroke={c}
        strokeWidth="3.96"
        fill="none"
      />
      <circle cx="30" cy="30" r="4.332" fill={c} />
    </svg>
  );
};

export default ParliaMark;
