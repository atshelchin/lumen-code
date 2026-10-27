// A transfer of 60 000 random bytes, rendered through the camera simulator (test/camera.js) into
// a video that Chrome plays as its webcam. Made once; delete test/out/cam.y4m to make it again.
import { execFileSync } from 'node:child_process';
import { existsSync, mkdirSync } from 'node:fs';
import path from 'node:path';
import { repo, camVideo } from '../playwright.config';

export default function setup() {
	if (existsSync(camVideo) && existsSync(camVideo + '.src')) return;
	mkdirSync(path.dirname(camVideo), { recursive: true });
	execFileSync(
		'node',
		['test/make-y4m.js', camVideo, '60000', '1', '100', '170', '8', '1280', '720'],
		{ cwd: repo, stdio: 'inherit' }
	);
}
