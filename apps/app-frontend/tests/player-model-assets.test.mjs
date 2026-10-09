import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { after, before, test } from 'node:test'

import { GLTFLoader } from 'three/examples/jsm/loaders/GLTFLoader.js'

const originalProgressEvent = globalThis.ProgressEvent
before(() => {
	globalThis.ProgressEvent = class extends Event {
		constructor(type, init) {
			super(type)
			Object.assign(this, init)
		}
	}
})
after(() => {
	if (originalProgressEvent) globalThis.ProgressEvent = originalProgressEvent
	else delete globalThis.ProgressEvent
})

for (const model of ['classic-player', 'slim-player']) {
	test(`${model} loads geometry and animations without missing placeholder textures`, async () => {
		const data = readFileSync(
			new URL(`../../../packages/assets/models/${model}.gltf`, import.meta.url),
			'utf8',
		)
		const json = JSON.parse(data)
		assert.ok(
			!json.images?.length,
			'textures are supplied by the selected skin and cape at runtime',
		)
		const gltf = await new Promise((resolve, reject) =>
			new GLTFLoader().parse(data, '', resolve, reject),
		)
		assert.ok(gltf.scene.getObjectByName('Head'))
		assert.ok(gltf.animations.length > 0)
		let meshes = 0
		let cape = false
		gltf.scene.traverse((object) => {
			if (!object.isMesh) return
			meshes++
			assert.ok(object.geometry.getAttribute('uv'))
			if (object.material.name === 'cape') cape = true
		})
		assert.ok(meshes > 0)
		assert.equal(cape, true)
	})
}
