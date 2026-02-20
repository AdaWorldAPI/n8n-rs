import { mock } from 'jest-mock-extended';
import type { ILoadOptionsFunctions } from 'n8n-workflow';

import { modelSearch } from './listSearch';
import * as transport from '../transport';

const mockResponse = {
	data: [
		{
			id: 'claude-opus-4-5-20251101',
		},
		{
			id: 'claude-opus-4-5-20251101',
		},
	],
};

describe('Anthropic -> listSearch', () => {
	const mockExecuteFunctions = mock<ILoadOptionsFunctions>();
	const apiRequestMock = jest.spyOn(transport, 'apiRequest');

	beforeEach(() => {
		jest.clearAllMocks();
	});

	describe('modelSearch', () => {
		it('should return all models', async () => {
			apiRequestMock.mockResolvedValue(mockResponse);

			const result = await modelSearch.call(mockExecuteFunctions);

			expect(result).toEqual({
				results: [
					{
						name: 'claude-opus-4-5-20251101',
						value: 'claude-opus-4-5-20251101',
					},
					{
						name: 'claude-opus-4-5-20251101',
						value: 'claude-opus-4-5-20251101',
					},
				],
			});
		});

		it('should return filtered models', async () => {
			apiRequestMock.mockResolvedValue(mockResponse);

			const result = await modelSearch.call(mockExecuteFunctions, 'sonnet');

			expect(result).toEqual({
				results: [
					{
						name: 'claude-opus-4-5-20251101',
						value: 'claude-opus-4-5-20251101',
					},
				],
			});
		});
	});
});
