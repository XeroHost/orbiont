export type { FeatureConfig } from '../core/abstract-feature'
export type { AuthConfig } from '../features/auth'
export type { BaseUrlConfig, ClientConfig, RequestHooks } from './client'
export type { ApiErrorData, ModrinthErrorResponse } from './errors'
export { isModrinthErrorResponse } from './errors'
export type { HttpMethod, RequestContext, RequestOptions, ResponseData } from './request'
export type {
	UploadHandle,
	UploadMetadata,
	UploadProgress,
	UploadRequestOptions,
	UploadState,
} from './upload'
