import { defineConfig } from '@playwright/test';
import path from 'node:path';

// The receiver's camera is Chrome's fake camera playing a recorded transfer (e2e/setup.ts makes it).
export const repo = path.resolve(import.meta.dirname, '../..');
export const camVideo = path.join(repo, 'test/out/cam.y4m');

export default defineConfig({
	webServer: {
		command: 'bun run build && bun run preview',
		port: 4173,
		reuseExistingServer: !process.env.CI,
		timeout: 180_000
	},
	testMatch: '**/*.e2e.{ts,js}',
	globalSetup: './e2e/setup.ts',
	timeout: 120_000,
	use: {
		baseURL: 'http://localhost:4173',
		launchOptions: {
			args: [
				'--use-fake-device-for-media-stream',
				`--use-file-for-fake-video-capture=${camVideo}`,
				'--use-fake-ui-for-media-stream'
			]
		}
	}
});
