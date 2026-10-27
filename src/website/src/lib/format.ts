export function fmtBytes(n: number): string {
	if (n < 1024) return `${Math.round(n)} B`;
	if (n < 1048576) return `${(n / 1024).toFixed(n < 10240 ? 1 : 0)} KB`;
	return `${(n / 1048576).toFixed(1)} MB`;
}

export function fmtTime(s: number): string {
	if (!isFinite(s) || s < 0) return '–';
	if (s < 60) return `${Math.ceil(s)}s`;
	return `${Math.floor(s / 60)}m${String(Math.ceil(s % 60)).padStart(2, '0')}s`;
}
