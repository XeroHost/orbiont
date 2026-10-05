export interface OptifineReference {
	minecraftVersion: string
	version: string
	installerSha256: string
}

export function matchesOptifineSelection(
	reference: OptifineReference,
	gameVersion: string,
	loader: string,
	expected?: OptifineReference | null,
): boolean {
	return (
		loader === 'vanilla' &&
		reference.minecraftVersion === gameVersion &&
		(!expected ||
			(reference.minecraftVersion === expected.minecraftVersion &&
				reference.version === expected.version &&
				reference.installerSha256 === expected.installerSha256))
	)
}
