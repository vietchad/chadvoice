// ChadVoice waveform mark. Keeps the original component name and viewBox so
// call sites are unchanged; inherits text color via fill-text like the old
// hand mark did.
const HandyHand = ({
  width,
  height,
}: {
  width?: number | string;
  height?: number | string;
}) => (
  <svg
    width={width || 126}
    height={height || 135}
    viewBox="0 0 126 135"
    className="fill-text"
    xmlns="http://www.w3.org/2000/svg"
  >
    <rect x="14" y="51" width="14" height="33" rx="7" />
    <rect x="41" y="34" width="14" height="67" rx="7" />
    <rect x="68" y="19" width="14" height="97" rx="7" />
    <rect x="95" y="43" width="14" height="49" rx="7" />
  </svg>
);

export default HandyHand;
