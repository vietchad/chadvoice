// ChadVoice chiseled waveform mark (matches the app icon). Keeps the original
// component name and viewBox so call sites are unchanged; inherits text color
// via fill-text like the old hand mark did.
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
    <polygon points="7,53.5 23,47.5 23,81.5 7,87.5" />
    <polygon points="31,39.5 47,33.5 47,95.5 31,101.5" />
    <polygon points="55,23.5 71,17.5 71,111.5 55,117.5" />
    <polygon points="79,34.5 95,28.5 95,100.5 79,106.5" />
    <polygon points="103,49.5 119,43.5 119,85.5 103,91.5" />
  </svg>
);

export default HandyHand;
