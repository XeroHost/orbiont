import type { ApiErrorData, ApiErrorResponse } from '../types/errors'
import { isApiErrorResponse } from '../types/errors'

/**
 * Base error class for all Modrinth API errors
 */
export class ApiError extends Error {
	/**
	 * HTTP status code (if available)
	 */
	readonly statusCode?: number

	/**
	 * Original error that was caught
	 */
	readonly originalError?: Error

	/**
	 * Response data from the API (if available)
	 */
	readonly responseData?: unknown

	/**
	 * Error context (e.g., module name, operation being performed)
	 */
	readonly context?: string

	constructor(message: string, data?: ApiErrorData) {
		super(message)
		this.name = 'ApiError'

		this.statusCode = data?.statusCode
		this.originalError = data?.originalError
		this.responseData = data?.responseData
		this.context = data?.context

		// Maintains proper stack trace for where our error was thrown (only available on V8)
		if (Error.captureStackTrace) {
			Error.captureStackTrace(this, ApiError)
		}
	}

	/**
	 * Create a ApiError from an unknown error
	 */
	static fromUnknown(error: unknown, context?: string): ApiError {
		if (error instanceof ApiError) {
			return error
		}

		if (error instanceof Error) {
			return new ApiError(error.message, {
				originalError: error,
				context,
			})
		}

		return new ApiError(String(error), { context })
	}
}

/**
 * Error class for structured (V1) API error responses
 * Extends ApiError with V1 error response parsing
 */
export class ApiServerError extends ApiError {
	/**
	 * V1 error information (if available)
	 */
	readonly v1Error?: ApiErrorResponse

	constructor(message: string, data?: ApiErrorData & { v1Error?: ApiErrorResponse }) {
		// If we have a V1 error, format the message nicely
		let errorMessage = message
		if (data?.v1Error) {
			errorMessage = `[${data.v1Error.error}] ${data.v1Error.description}`
			if (data.v1Error.context) {
				errorMessage = `${data.v1Error.context}: ${errorMessage}`
			}
		}

		super(errorMessage, data)
		this.name = 'ApiServerError'
		this.v1Error = data?.v1Error

		if (Error.captureStackTrace) {
			Error.captureStackTrace(this, ApiServerError)
		}
	}

	/**
	 * Create a ApiServerError from response data
	 */
	static fromResponse(statusCode: number, responseData: unknown, context?: string): ApiServerError {
		const v1Error = isApiErrorResponse(responseData) ? responseData : undefined

		let message = `HTTP ${statusCode}`
		if (v1Error) {
			message = v1Error.description
		} else if (typeof responseData === 'string') {
			message = responseData
		}

		return new ApiServerError(message, {
			statusCode,
			responseData,
			context,
			v1Error,
		})
	}

	/**
	 * Create a ApiServerError from an unknown error
	 */
	static fromUnknown(error: unknown, context?: string): ApiServerError {
		if (error instanceof ApiServerError) {
			return error
		}

		if (error instanceof ApiError) {
			return new ApiServerError(error.message, {
				statusCode: error.statusCode,
				originalError: error.originalError,
				responseData: error.responseData,
				context: context ?? error.context,
			})
		}

		if (error instanceof Error) {
			return new ApiServerError(error.message, {
				originalError: error,
				context,
			})
		}

		return new ApiServerError(String(error), { context })
	}
}
